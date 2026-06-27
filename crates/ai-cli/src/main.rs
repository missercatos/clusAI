use ai_core::prelude::*;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DEFAULT_AUTO_ROUNDS: usize = 10;
const HISTORY_FILE: &str = ".clusai_history";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "ai_cli=info,ai_core=error".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let config = AgentConfig::load().map_err(|e| anyhow::anyhow!("config error: {e}"))?;
    let providers: Vec<String> = config.providers.iter().map(|p| p.id.clone()).collect();
    let mode = format!("{:?}", config.agent.default_mode);

    let agent = AgentHandle::spawn(config).await?;
    let mut rx = agent.subscribe();

    loop {
        match rx.try_recv() {
            Ok(KernelOutput::Error { message, .. }) => {
                eprintln!("fatal: {message}");
                return Err(anyhow::anyhow!("{message}"));
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    eprintln!("ai-cli v0.1.0 — AI coding assistant");
    eprintln!("providers: {} | mode: {}", providers.join(", "), mode.to_lowercase());
    eprintln!("Type /help for commands, Ctrl+D to quit\n");

    let (line_tx, mut line_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Background: line reader — rustyline when TTY, plain stdin when piped
    {
        let stop_flag = stop_flag.clone();
        let is_tty = atty::is(atty::Stream::Stdin);
        tokio::task::spawn_blocking(move || {
            if is_tty {
                run_rustyline(line_tx, stop_flag);
            } else {
                run_stdin_pipe(line_tx, stop_flag);
            }
        });
    }

    let mut auto_mode: Option<usize> = None;
    let mut pending_auto_msg: Option<String> = None;
    let mut current_model = String::new();
    let mut in_roundtable = false;

    loop {
        if auto_mode.is_some() && stop_flag.swap(false, Ordering::Relaxed) {
            auto_mode = None;
            pending_auto_msg = None;
            current_model.clear();
            in_roundtable = false;
            println!("\n[auto] 用户停止");
            continue;
        }

        if let Some(msg) = pending_auto_msg.take() {
            if let Err(e) = agent.send_message(&msg) {
                eprintln!("error: {e}");
                auto_mode = None;
                continue;
            }
        }

        tokio::select! {
            Some(line) = line_rx.recv() => {
                if line == "/stop" { continue; }
                if line.is_empty() { continue; }

                if line.starts_with('/') {
                    handle_command(&line, &agent, &mut auto_mode).await;
                    continue;
                }

                if let Err(e) = agent.send_message(&line) {
                    eprintln!("error: {e}");
                    continue;
                }

                current_model.clear();
                in_roundtable = false;
            }

            Ok(event) = rx.recv() => {
                let done = render_event(event, &mut current_model, &mut in_roundtable);

                if done {
                    match auto_mode {
                        Some(ref mut remaining) => {
                            *remaining -= 1;
                            if *remaining == 0 {
                                auto_mode = None;
                                println!("\n[auto] 达到最大轮数，停止");
                            } else {
                                pending_auto_msg = Some(
                                    "请继续你们的对话，回应上一位发言者。".into()
                                );
                                current_model.clear();
                                in_roundtable = false;
                            }
                        }
                        None => { println!(); }
                    }
                }
            }
        }
    }
}

fn render_event(event: KernelOutput, current_model: &mut String, in_roundtable: &mut bool) -> bool {
    match event {
        KernelOutput::StreamStart { model, .. } => {
            *current_model = model;
            false
        }
        KernelOutput::TextDelta { content, .. } => {
            print!("{content}");
            io::stdout().flush().ok();
            false
        }
        KernelOutput::StreamEnd { .. } => false,

        KernelOutput::RoundStart { model, provider_id } => {
            println!("\n[{} ({})]:", provider_id, model);
            *current_model = model.clone();
            *in_roundtable = true;
            io::stdout().flush().ok();
            false
        }
        KernelOutput::RoundEnd { .. } => { println!(); false }
        KernelOutput::RoundtableComplete => {
            println!("\n--- roundtable complete ---");
            *in_roundtable = false;
            true
        }

        KernelOutput::MessageComplete { .. } => !*in_roundtable,

        KernelOutput::SessionExported { content, format } => {
            let ext = format.extension();
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let fname = format!("chat_{}.{}", ts, ext);
            match std::fs::write(&fname, &content) {
                Ok(_) => println!("\n[save] 对话已保存到 {fname} (格式: {})", format.display_name()),
                Err(e) => eprintln!("\n[save] 保存失败: {e}"),
            }
            false
        }

        KernelOutput::SessionSaved { id } => {
            println!("\n[session] 会话已保存: {id}");
            false
        }
        KernelOutput::SessionLoaded { id, message_count } => {
            println!("\n[session] 已加载: {id} ({message_count} 条消息)");
            false
        }
        KernelOutput::SessionList { sessions } => {
            println!("\n--- 存档列表 ---");
            if sessions.is_empty() { println!("  (空)"); }
            for s in &sessions {
                println!("  {} | {} | {} 条消息", s.id, s.topic, s.message_count);
            }
            false
        }
        KernelOutput::SessionDeleted { id } => {
            println!("\n[session] 已删除: {id}");
            false
        }

        KernelOutput::PermissionRequest { call_id: _, tool_name, args_preview, risk_level } => {
            eprintln!("\n[!] tool request: {tool_name}({args_preview}) [risk: {risk_level:?}]");
            eprintln!("auto-approved");
            false
        }
        KernelOutput::ToolCallStart { tool_name, args_preview, .. } => {
            eprintln!("[tool] {tool_name}({args_preview})");
            false
        }
        KernelOutput::ToolCallEnd { tool_name, succeeded, output_preview, .. } => {
            let status = if succeeded { "ok" } else { "FAIL" };
            eprintln!("[tool] {tool_name} → {status}: {output_preview}");
            false
        }

        KernelOutput::Blueprint(bp) => {
            match bp {
                BlueprintEvent::PlanCreated { task, stages, .. } => {
                    println!("\n[blueprint] task: {task}");
                    for s in &stages {
                        println!("  - {}: {} (assigned: {})", s.name, s.description, s.assignee);
                    }
                }
                BlueprintEvent::StageStarted { stage, assignee } => {
                    println!("\n[blueprint] stage '{stage}' started by {assignee}");
                }
                BlueprintEvent::DraftFileUpdated { file, content_snippet, author } => {
                    println!("[blueprint] {file} ← {author}\n  {content_snippet}");
                }
                BlueprintEvent::DiffPreview { file, diff } => {
                    println!("[blueprint] diff for {file}:\n{diff}");
                }
                BlueprintEvent::StageCompleted { stage } => {
                    println!("[blueprint] stage '{stage}' complete");
                }
                BlueprintEvent::BlueprintCompleted { files_changed } => {
                    println!("[blueprint] complete! files changed: {:?}", files_changed);
                }
                BlueprintEvent::ReviewComment { file, comment, reviewer } => {
                    println!("[review] {file}: {comment} — {reviewer}");
                }
                BlueprintEvent::BlueprintFailed { stage, error } => {
                    eprintln!("[blueprint] failed at '{stage}': {error}");
                }
            }
            false
        }

        KernelOutput::StatusChanged(state) => {
            eprintln!("\n[status] model={}, processing={}, history={}",
                state.active_model.as_deref().unwrap_or("none"),
                state.is_processing,
                state.history_len,
            );
            false
        }

        KernelOutput::Error { message, kind } => {
            eprintln!("\n[{:?}] {message}", kind);
            true
        }
    }
}

async fn handle_command(line: &str, agent: &AgentHandle, auto_mode: &mut Option<usize>) {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd = parts[0].trim_start_matches('/').trim();
    let arg = parts.get(1).copied().unwrap_or("");

    match cmd {
        "help" => {
            println!("Commands:");
            println!("  /help              Show this help");
            println!("  /status            Show agent status");
            println!("  /history           Show conversation history");
            println!("  /clear             Clear conversation history");
            println!("  /model <id>        Switch model");
            println!("  /mode <mode>       Switch mode (single|roundtable|blueprint)");
            println!("  /auto [n]          Auto-dialogue for n rounds (default {})", DEFAULT_AUTO_ROUNDS);
            println!("  /stop              Stop auto-dialogue");
            println!("  /save [fmt]        Save conversation (txt|json|md, default txt)");
            println!("  /session-save [n]  Save session for later restore");
            println!("  /session-load <id> Load a saved session");
            println!("  /session-list      List saved sessions");
            println!("  /quit              Exit");
        }
        "status" => { let _ = agent.send_action(UserAction::ShowStatus); }
        "history" => { let _ = agent.send_action(UserAction::ShowHistory); }
        "clear" => {
            let _ = agent.send_action(UserAction::ClearHistory);
            println!("History cleared.");
        }
        "model" if !arg.is_empty() => {
            let _ = agent.send_action(UserAction::SwitchModel(arg.to_string()));
        }
        "mode" if !arg.is_empty() => {
            let mode = match arg {
                "single" => ChatMode::Single,
                "roundtable" | "round" => ChatMode::Roundtable,
                "blueprint" | "bp" | "plan" => ChatMode::Blueprint,
                _ => { eprintln!("unknown mode: {arg}"); return; }
            };
            let _ = agent.send_action(UserAction::ToggleMode(mode));
        }
        "auto" => {
            if *auto_mode != None { *auto_mode = None; return; }
            let max: usize = arg.parse().unwrap_or(DEFAULT_AUTO_ROUNDS).min(50);
            *auto_mode = Some(max);
            println!("\n[auto] 进入自动对话模式（{}轮），发送首条消息开始对话，输入 /stop 停止", max);
        }
        "stop" => {
            if auto_mode.is_some() {
                *auto_mode = None;
                println!("auto-dialogue stopped.");
            }
        }
        "save" => {
            let fmt = if arg.is_empty() { "txt" } else { arg };
            let _ = agent.send_action(UserAction::ExportSession(fmt.to_string()));
        }
        "session-save" | "ss" => {
            let _ = agent.send_action(UserAction::SaveSession(arg.to_string()));
        }
        "session-load" | "sl" => {
            let _ = agent.send_action(UserAction::LoadSession(arg.to_string()));
        }
        "session-list" | "sessions" => {
            let _ = agent.send_action(UserAction::ListSessions);
        }
        "quit" | "exit" => {
            let _ = agent.send_action(UserAction::SaveSession(String::new()));
            std::process::exit(0);
        }
        _ => { eprintln!("unknown command: {cmd}"); }
    }
}

// ── line readers ──

fn run_rustyline(
    line_tx: tokio::sync::mpsc::UnboundedSender<String>,
    stop_flag: Arc<AtomicBool>,
) {
    let mut rl = match rustyline::Editor::<(), rustyline::history::DefaultHistory>::new() {
        Ok(e) => e,
        Err(_) => return,
    };
    let _ = rl.load_history(HISTORY_FILE);
    loop {
        match rl.readline("user: ") {
            Ok(line) => {
                let trimmed = line.trim().to_string();
                if !trimmed.is_empty() {
                    let _ = rl.add_history_entry(&trimmed);
                    let _ = rl.save_history(HISTORY_FILE);
                }
                if trimmed == "/stop" {
                    stop_flag.store(true, Ordering::Relaxed);
                }
                if line_tx.send(trimmed).is_err() {
                    break;
                }
            }
            Err(rustyline::error::ReadlineError::Eof)
            | Err(rustyline::error::ReadlineError::Interrupted) => break,
            Err(_) => break,
        }
    }
}

fn run_stdin_pipe(
    line_tx: tokio::sync::mpsc::UnboundedSender<String>,
    stop_flag: Arc<AtomicBool>,
) {
    for line in io::stdin().lines() {
        let l = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = l.trim().to_string();
        if trimmed == "/stop" {
            stop_flag.store(true, Ordering::Relaxed);
        }
        if line_tx.send(trimmed).is_err() {
            break;
        }
    }
}
