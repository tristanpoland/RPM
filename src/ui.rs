use colored::*;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color, ContentArrangement, Table};
use crate::process::{ProcessInfo, ProcessStatus};
use std::time::Duration;

pub struct TableFormatter;

impl TableFormatter {
    pub fn format_process_list(processes: &[&ProcessInfo]) -> String {
        if processes.is_empty() {
            return "No processes running".bright_yellow().to_string();
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("NAME").fg(Color::Cyan).add_attribute(comfy_table::Attribute::Bold),
                Cell::new("ID").fg(Color::Cyan).add_attribute(comfy_table::Attribute::Bold),
                Cell::new("STATUS").fg(Color::Cyan).add_attribute(comfy_table::Attribute::Bold),
                Cell::new("CPU").fg(Color::Cyan).add_attribute(comfy_table::Attribute::Bold),
                Cell::new("MEMORY").fg(Color::Cyan).add_attribute(comfy_table::Attribute::Bold),
                Cell::new("RESTARTS").fg(Color::Cyan).add_attribute(comfy_table::Attribute::Bold),
                Cell::new("UPTIME").fg(Color::Cyan).add_attribute(comfy_table::Attribute::Bold),
            ]);

        for process in processes {
            let status_cell = Self::format_status_cell(&process.status);
            let cpu_cell = Cell::new(format!("{:.1}%", process.cpu_usage))
                .fg(Self::get_cpu_color(process.cpu_usage));
            let memory_cell = Cell::new(Self::format_memory(process.memory_usage))
                .fg(Self::get_memory_color(process.memory_usage));
            let uptime_cell = Cell::new(Self::format_duration_since(process.started_at));
            
            table.add_row(vec![
                Cell::new(&process.name).fg(Color::White),
                Cell::new(&process.id[..8]).fg(Color::DarkGrey), // Show only first 8 chars of UUID
                status_cell,
                cpu_cell,
                memory_cell,
                Cell::new(process.restarts.to_string()).fg(if process.restarts > 0 { Color::Yellow } else { Color::DarkGrey }),
                uptime_cell,
            ]);
        }

        table.to_string()
    }

    pub fn format_process_details(process: &ProcessInfo) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("{}\n", "Process Information".bright_cyan().bold()));
        output.push_str(&format!("{}────────────────────\n", "".bright_cyan()));
        
        output.push_str(&format!("{:<12} {}\n", "Name:".bright_white(), process.name.bright_yellow()));
        output.push_str(&format!("{:<12} {}\n", "ID:".bright_white(), process.id.bright_blue()));
        output.push_str(&format!("{:<12} {}\n", "Status:".bright_white(), Self::format_status_text(&process.status)));
        output.push_str(&format!("{:<12} {}\n", "PID:".bright_white(), 
            process.pid.map_or("N/A".dimmed().to_string(), |p| p.to_string().bright_green().to_string())));
        output.push_str(&format!("{:<12} {}\n", "CPU:".bright_white(), 
            format!("{:.1}%", process.cpu_usage).color(Self::get_cpu_color_name(process.cpu_usage))));
        output.push_str(&format!("{:<12} {}\n", "Memory:".bright_white(), 
            Self::format_memory(process.memory_usage).color(Self::get_memory_color_name(process.memory_usage))));
        output.push_str(&format!("{:<12} {}\n", "Command:".bright_white(), process.command.bright_white()));
        output.push_str(&format!("{:<12} {}\n", "Started:".bright_white(), 
            process.started_at.format("%Y-%m-%d %H:%M:%S UTC").to_string().bright_magenta()));
        output.push_str(&format!("{:<12} {}\n", "Restarts:".bright_white(), 
            process.restarts.to_string().color(if process.restarts > 0 { "yellow" } else { "bright_black" })));
        output.push_str(&format!("{:<12} {}\n", "Uptime:".bright_white(), Self::format_duration_since(process.started_at).bright_green()));
        
        if let Some(cwd) = &process.config.cwd {
            output.push_str(&format!("{:<12} {}\n", "Directory:".bright_white(), cwd.bright_blue()));
        }
        
        if !process.config.env.is_empty() {
            output.push_str(&format!("{:<12}\n", "Environment:".bright_white()));
            for (key, value) in &process.config.env {
                output.push_str(&format!("  {}: {}\n", key.bright_cyan(), value.white()));
            }
        }

        output
    }

    fn format_status_cell(status: &ProcessStatus) -> Cell {
        match status {
            ProcessStatus::Running => Cell::new("●  running").fg(Color::Green),
            ProcessStatus::Stopped => Cell::new("○  stopped").fg(Color::Red),
            ProcessStatus::Errored => Cell::new("✕  errored").fg(Color::DarkRed),
            ProcessStatus::Restarting => Cell::new("↻  restarting").fg(Color::Yellow),
        }
    }

    fn format_status_text(status: &ProcessStatus) -> ColoredString {
        match status {
            ProcessStatus::Running => "●  running".bright_green(),
            ProcessStatus::Stopped => "○  stopped".bright_red(),
            ProcessStatus::Errored => "✕  errored".red(),
            ProcessStatus::Restarting => "↻  restarting".bright_yellow(),
        }
    }

    fn get_cpu_color(cpu: f64) -> Color {
        match cpu {
            x if x > 80.0 => Color::Red,
            x if x > 50.0 => Color::Yellow,
            x if x > 20.0 => Color::Blue,
            _ => Color::Green,
        }
    }

    fn get_cpu_color_name(cpu: f64) -> &'static str {
        match cpu {
            x if x > 80.0 => "red",
            x if x > 50.0 => "yellow",
            x if x > 20.0 => "blue",
            _ => "green",
        }
    }

    fn get_memory_color(memory: u64) -> Color {
        let memory_mb = memory / 1024 / 1024;
        match memory_mb {
            x if x > 1000 => Color::Red,
            x if x > 500 => Color::Yellow,
            x if x > 100 => Color::Blue,
            _ => Color::Green,
        }
    }

    fn get_memory_color_name(memory: u64) -> &'static str {
        let memory_mb = memory / 1024 / 1024;
        match memory_mb {
            x if x > 1000 => "red",
            x if x > 500 => "yellow",
            x if x > 100 => "blue",
            _ => "green",
        }
    }

    fn format_memory(bytes: u64) -> String {
        let mb = bytes as f64 / 1024.0 / 1024.0;
        if mb >= 1024.0 {
            format!("{:.1}GB", mb / 1024.0)
        } else {
            format!("{:.1}MB", mb)
        }
    }

    fn format_duration_since(start: chrono::DateTime<chrono::Utc>) -> String {
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(start);
        
        if let Ok(std_duration) = duration.to_std() {
            Self::format_duration(std_duration)
        } else {
            "N/A".to_string()
        }
    }

    fn format_duration(duration: Duration) -> String {
        let total_seconds = duration.as_secs();
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if days > 0 {
            format!("{}d {}h", days, hours)
        } else if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }
}

pub struct ProgressIndicator;

impl ProgressIndicator {
    pub fn show_spinner(message: &str) -> indicatif::ProgressBar {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner:.cyan} {msg}")
                .unwrap()
        );
        pb.set_message(message.to_string());
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    }

    pub fn show_progress(total: u64, message: &str) -> indicatif::ProgressBar {
        let pb = indicatif::ProgressBar::new(total);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message(message.to_string());
        pb
    }
}

pub fn print_success(message: &str) {
    println!("{} {}", "✓".bright_green().bold(), message.bright_white());
}

pub fn print_error(message: &str) {
    eprintln!("{} {}", "✗".bright_red().bold(), message.bright_white());
}

pub fn print_warning(message: &str) {
    println!("{} {}", "⚠".bright_yellow().bold(), message.bright_white());
}

pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".bright_blue().bold(), message.bright_white());
}

pub fn print_header(title: &str) {
    let len = title.len() + 4;
    let border = "═".repeat(len);
    
    println!("{}", border.bright_cyan());
    println!("  {}  ", title.bright_cyan().bold());
    println!("{}", border.bright_cyan());
}