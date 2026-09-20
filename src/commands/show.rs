use anyhow::Result;
use colored::Colorize;

use crate::cli::ShowArgs;
use crate::object::{Event, resolve_id};
use crate::repo::Repo;
use crate::store::Store;

pub fn run(args: ShowArgs) -> Result<()> {
    let repo = Repo::discover()?;
    let store = Store::new(&repo);
    let full = resolve_id(&repo.objects_dir(), &args.id)?;
    let ev = store.read_event(&full)?;

    if args.json {
        let mut v = serde_json::to_value(&ev)?;
        if let Some(obj) = v.as_object_mut() {
            obj.insert("id".into(), serde_json::Value::String(full));
        }
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    print_terminal(&full, &ev);
    Ok(())
}

fn print_terminal(id: &str, ev: &Event) {
    println!("{} {}", "event".yellow().bold(), id.yellow());
    if let Some(p) = &ev.parent {
        println!("  parent:   {}", &p[..10.min(p.len())]);
    }
    if let Some(a) = &ev.agent {
        println!("  agent:    {}", a);
    }
    if let Some(m) = &ev.model {
        println!("  model:    {}", m);
    }
    if let Some(t) = &ev.tool {
        println!("  tool:     {}", t.cyan());
    }
    if let Some(e) = &ev.evidence {
        println!("  evidence: {}", e.describe());
    } else {
        println!("  evidence: {}", "unrecorded (older binary)".bright_black());
    }
    println!("  date:     {}", ev.created_at);
    println!(
        "  pre:      {}",
        &ev.pre_snapshot[..10.min(ev.pre_snapshot.len())]
    );
    println!(
        "  post:     {}",
        &ev.post_snapshot[..10.min(ev.post_snapshot.len())]
    );
    if let Some(c) = ev.exit_code {
        println!("  exit:     {}", c);
    }
    match (ev.tokens_in, ev.tokens_out) {
        (None, None) => {}
        (i, o) => println!(
            "  tokens:   {} in · {} out",
            i.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
            o.map(|n| n.to_string()).unwrap_or_else(|| "?".into())
        ),
    }
    if let Some(c) = ev.cost_usd {
        println!("  cost:     ${:.4}", c);
    }
    if !ev.reads.is_empty() {
        println!("  reads:    {}", ev.reads.join(", "));
    }
    if !ev.writes.is_empty() {
        println!("  writes:   {}", ev.writes.join(", "));
    }
    if let Some(m) = &ev.message {
        println!();
        println!("  {}", m);
    }
    if let Some(p) = &ev.prompt {
        println!();
        println!("  {}", "prompt".bold());
        for line in p.lines() {
            println!("    {}", line);
        }
    }
    if let Some(r) = &ev.reasoning {
        println!();
        println!("  {}", "reasoning".bold());
        for line in r.lines() {
            println!("    {}", line);
        }
    }
}
