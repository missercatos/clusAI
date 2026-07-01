//! C FFI for ai-core.
//!
//! Build: `cargo build -p ai-ffi --release`
//! Output: `target/release/libai_ffi.so`
//!
//! ## API (synchronous — ai_think blocks until complete)
//!
//! ```c
//! void* h = ai_init("./.clusai.toml");
//! ai_think(h, "Hello");
//! while (!ai_done(h)) {         // always true right after ai_think
//!     char buf[4096];
//!     int n = ai_poll(h, buf, sizeof(buf));
//!     if (n > 0) printf("%.*s", n, buf);
//!     else break;
//! }
//! ai_free(h);
//! ```
//!
//! For true streaming, use `ai-serve` over STDIO.

use std::collections::VecDeque;
use std::ffi::{c_char, c_int, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use ai_core::config::AgentConfig;
use ai_core::interface::agent_handle::AgentHandle;
use ai_core::interface::output::KernelOutput;
use serde_json::json;

pub struct Handle {
    agent: AgentHandle,
    rt: tokio::runtime::Runtime,
    queue: Mutex<VecDeque<String>>,
    done: AtomicBool,
}

/// Initialise the kernel. Returns opaque handle or null on error.
#[unsafe(no_mangle)]
pub extern "C" fn ai_init(config_path: *const c_char) -> *mut Handle {
    let path = ptr_to_str(config_path);
    let config = match if path.is_empty() {
        AgentConfig::load()
    } else {
        AgentConfig::load_from(std::path::Path::new(&path))
    } {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[ai-ffi] config error: {e}");
            return std::ptr::null_mut();
        }
    };

    let rt = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ai-ffi] runtime error: {e}");
            return std::ptr::null_mut();
        }
    };

    let agent = match rt.block_on(AgentHandle::spawn(config)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[ai-ffi] agent error: {e}");
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(Handle {
        agent,
        rt,
        queue: Mutex::new(VecDeque::new()),
        done: AtomicBool::new(true),
    }))
}

/// Send a prompt and drain all events into the internal queue.
/// Blocks until the agent finishes (or errors).
/// Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn ai_think(handle: *mut Handle, prompt: *const c_char) -> c_int {
    let Some(h) = (unsafe { handle.as_mut() }) else { return 1 };
    let text = ptr_to_str(prompt);

    h.done.store(false, Ordering::Release);

    if h.agent.send_message(&text).is_err() {
        h.queue.lock().unwrap().push_back(
            json!({"type":"error","message":"agent channel closed"}).to_string(),
        );
        h.done.store(true, Ordering::Release);
        return 1;
    }

    let result = h.rt.block_on(async {
        loop {
            match h.agent.recv().await {
                Ok(out) => {
                    let event = kernel_to_json(out);
                    let is_terminal = matches!(
                        event.get("type").and_then(|v| v.as_str()),
                        Some("agent_finished" | "error")
                    );
                    h.queue.lock().unwrap().push_back(event.to_string());
                    if is_terminal {
                        return true;
                    }
                }
                Err(e) => {
                    h.queue.lock().unwrap().push_back(
                        json!({"type":"error","message":e.to_string()}).to_string(),
                    );
                    return false;
                }
            }
        }
    });

    h.done.store(true, Ordering::Release);
    if result { 0 } else { 1 }
}

/// Pop next event into buf. Returns bytes written (0 = no event).
#[unsafe(no_mangle)]
pub extern "C" fn ai_poll(handle: *mut Handle, buf: *mut c_char, len: c_int) -> c_int {
    let Some(h) = (unsafe { handle.as_mut() }) else { return -1 };
    let mut q = h.queue.lock().unwrap();
    let Some(event) = q.pop_front() else { return 0 };
    let bytes = event.as_bytes();
    let copy = bytes.len().min(len as usize);
    if copy > 0 {
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy); }
    }
    copy as c_int
}

/// Returns 1 when the current think is done (all events drained).
#[unsafe(no_mangle)]
pub extern "C" fn ai_done(handle: *const Handle) -> c_int {
    let Some(h) = (unsafe { handle.as_ref() }) else { return 1 };
    h.done.load(Ordering::Acquire) as c_int
}

/// Free the handle.
#[unsafe(no_mangle)]
pub extern "C" fn ai_free(handle: *mut Handle) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ai_version() -> *const c_char {
    static V: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();
    V.as_ptr() as *const c_char
}

// ─── helpers ─────────────────────────────────────────────────────────────

fn ptr_to_str(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn kernel_to_json(out: KernelOutput) -> serde_json::Value {
    match out {
        KernelOutput::TextDelta { content, model, .. } => {
            json!({"type":"text_delta","agent_id":model,"content":content})
        }
        KernelOutput::ToolCallStart { tool_name, args_preview, .. } => {
            json!({"type":"tool_call_start","tool_name":tool_name,"args_preview":args_preview})
        }
        KernelOutput::ToolCallEnd { tool_name, succeeded, output_preview, .. } => {
            json!({"type":"tool_call_end","tool_name":tool_name,"succeeded":succeeded,"output_preview":output_preview})
        }
        KernelOutput::MessageComplete { message } => {
            json!({"type":"agent_finished","content":message.content.unwrap_or_default()})
        }
        KernelOutput::Error { message, .. } => json!({"type":"error","message":message}),
        KernelOutput::RoundStart { provider_id, .. } => {
            json!({"type":"round_start","agent_id":provider_id})
        }
        KernelOutput::RoundEnd { provider_id, .. } => {
            json!({"type":"round_end","agent_id":provider_id})
        }
        KernelOutput::RoundtableComplete => json!({"type":"roundtable_complete"}),
        _ => json!({"type":"unknown"}),
    }
}
