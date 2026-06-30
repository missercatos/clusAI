use std::io::Write;

use anyhow::Context;
use tokio::io::{AsyncBufReadExt, BufReader};

use ai_core::config::AgentConfig;
use ai_core::interface::output::KernelOutput;
use ai_core::interface::agent_handle::AgentHandle;
use ai_core::interface::input::UserAction;
use ai_core::serve::protocol::{Event, Request, SessionMeta};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("ai_serve=info").init();

    let config = AgentConfig::load().context("failed to load config")?;
    let mut agent = AgentHandle::spawn(config).await?;

    eprintln!("[ai-serve] ready");

    let mut stdin = BufReader::new(tokio::io::stdin()).lines();

    while let Some(line) = stdin.next_line().await.context("stdin closed")? {
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                emit(&Event::Error { message: format!("invalid request: {e}") });
                continue;
            }
        };

        match req {
            Request::Think { agent: _, prompt } => {
                agent.send_message(&prompt)?;
                loop {
                    let out = agent.recv().await.context("agent channel closed")?;
                    let done = matches!(out, KernelOutput::MessageComplete { .. } | KernelOutput::Error { .. });
                    emit_kernel(out);
                    if done {
                        break;
                    }
                }
            }
            Request::SaveSession { id } => {
                agent.send_action(UserAction::SaveSession(id.unwrap_or_default()))?;
                drain_session_response(&mut agent).await?;
            }
            Request::LoadSession { id } => {
                agent.send_action(UserAction::LoadSession(id))?;
                drain_session_response(&mut agent).await?;
            }
            Request::ListSessions => {
                agent.send_action(UserAction::ListSessions)?;
                drain_session_response(&mut agent).await?;
            }
            Request::ClearHistory => {
                agent.send_action(UserAction::ClearHistory)?;
            }
            Request::ListAgents => {
                emit(&Event::Error { message: "list_agents not available in AgentHandle mode".into() });
            }
            Request::LoadConfig { path: _ } => {
                emit(&Event::Error { message: "load_config: hot reload not supported — restart ai-serve".into() });
            }
            Request::Quit => {
                emit(&Event::Bye);
                agent.shutdown();
                break;
            }
        }
    }

    Ok(())
}

fn emit(event: &Event) {
    let mut buf = serde_json::to_string(event).unwrap_or_default();
    buf.push('\n');
    let _ = std::io::stdout().write_all(buf.as_bytes());
    let _ = std::io::stdout().flush();
}

fn emit_kernel(out: KernelOutput) {
    let event = match out {
        KernelOutput::TextDelta { content, model, .. } => {
            Event::TextDelta { agent_id: model, content }
        }
        KernelOutput::ToolCallStart { tool_name, args_preview, .. } => {
            Event::ToolCallStart { agent_id: String::new(), tool_name, args_preview }
        }
        KernelOutput::ToolCallEnd { tool_name, succeeded, output_preview, .. } => {
            Event::ToolCallEnd { agent_id: String::new(), tool_name, succeeded, output_preview }
        }
        KernelOutput::MessageComplete { message } => {
            Event::AgentFinished { agent_id: String::new(), content: message.content.unwrap_or_default() }
        }
        KernelOutput::Error { message, .. } => {
            Event::Error { message }
        }
        KernelOutput::SessionSaved { id } => Event::SessionSaved { id },
        KernelOutput::SessionLoaded { id, message_count } => Event::SessionLoaded { id, message_count },
        KernelOutput::SessionList { sessions } => Event::SessionList {
            sessions: sessions.into_iter().map(SessionMeta::from).collect(),
        },
        _ => return, // skip unconsumed variants
    };
    emit(&event);
}

async fn drain_session_response(agent: &mut AgentHandle) -> anyhow::Result<()> {
    loop {
        let out = agent.recv().await?;
        let done = matches!(out, KernelOutput::SessionSaved { .. } | KernelOutput::SessionLoaded { .. } | KernelOutput::SessionList { .. } | KernelOutput::Error { .. });
        emit_kernel(out);
        if done {
            break;
        }
    }
    Ok(())
}
