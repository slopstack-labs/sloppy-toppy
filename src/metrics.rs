//! System metrics collection, wrapped around `sysinfo`.

use sysinfo::{ProcessesToUpdate, System};

/// How many points of per-core CPU history we keep for the sparkline-ish display.
const HISTORY_LEN: usize = 60;

/// A single process row shown in the table.
#[derive(Clone)]
pub struct ProcRow {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_bytes: u64,
}

/// A snapshot of everything the UI (and the LLM) cares about.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub cpu_overall: f32,
    pub per_core: Vec<f32>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub uptime_secs: u64,
    pub load_one: f64,
    pub top_procs: Vec<ProcRow>,
}

impl Snapshot {
    pub fn mem_frac(&self) -> f64 {
        if self.mem_total == 0 {
            0.0
        } else {
            self.mem_used as f64 / self.mem_total as f64
        }
    }

    pub fn swap_frac(&self) -> f64 {
        if self.swap_total == 0 {
            0.0
        } else {
            self.swap_used as f64 / self.swap_total as f64
        }
    }
}

/// Owns the `sysinfo::System` and a rolling history of overall CPU usage.
pub struct Collector {
    sys: System,
    pub cpu_history: Vec<u64>,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        // Prime the CPU counters; the first reading is always garbage.
        sys.refresh_cpu_usage();
        Collector {
            sys,
            cpu_history: Vec::with_capacity(HISTORY_LEN),
        }
    }

    /// Refresh all subsystems and return a fresh snapshot.
    pub fn sample(&mut self) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys
            .refresh_processes(ProcessesToUpdate::All, true);

        let per_core: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let cpu_overall = self.sys.global_cpu_usage();

        self.cpu_history.push(cpu_overall.round() as u64);
        if self.cpu_history.len() > HISTORY_LEN {
            let overflow = self.cpu_history.len() - HISTORY_LEN;
            self.cpu_history.drain(0..overflow);
        }

        let mut top_procs: Vec<ProcRow> = self
            .sys
            .processes()
            .iter()
            .map(|(pid, proc_)| ProcRow {
                pid: pid.as_u32(),
                name: proc_.name().to_string_lossy().into_owned(),
                cpu: proc_.cpu_usage(),
                mem_bytes: proc_.memory(),
            })
            .collect();
        top_procs.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
        top_procs.truncate(12);

        let load = System::load_average();

        Snapshot {
            cpu_overall,
            per_core,
            mem_used: self.sys.used_memory(),
            mem_total: self.sys.total_memory(),
            swap_used: self.sys.used_swap(),
            swap_total: self.sys.total_swap(),
            uptime_secs: System::uptime(),
            load_one: load.one,
            top_procs,
        }
    }
}

/// Human-friendly bytes (KiB/MiB/GiB).
pub fn fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
