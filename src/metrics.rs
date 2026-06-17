//! System metrics collection, wrapped around `sysinfo`.

use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System};

const HISTORY_LEN: usize = 60;

#[derive(Clone)]
pub struct ProcRow {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem_bytes: u64,
}

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
    pub net_rx_bps: u64,
    pub net_tx_bps: u64,
    pub disk_read_bps: u64,
    pub disk_write_bps: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    pub cpu_temp: Option<f32>,
}

impl Snapshot {
    pub fn mem_frac(&self) -> f64 {
        if self.mem_total == 0 { 0.0 } else { self.mem_used as f64 / self.mem_total as f64 }
    }
    pub fn swap_frac(&self) -> f64 {
        if self.swap_total == 0 { 0.0 } else { self.swap_used as f64 / self.swap_total as f64 }
    }
    pub fn disk_frac(&self) -> f64 {
        if self.disk_total == 0 { 0.0 } else { self.disk_used as f64 / self.disk_total as f64 }
    }
}

pub struct Collector {
    sys: System,
    networks: Networks,
    disks: Disks,
    components: Components,
    pub cpu_history: Vec<u64>,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_cpu_usage();
        Collector {
            sys,
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            cpu_history: Vec::with_capacity(HISTORY_LEN),
        }
    }

    pub fn sample(&mut self) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.networks.refresh(false);
        self.disks.refresh(false);
        self.components.refresh(false);

        let per_core: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let cpu_overall = self.sys.global_cpu_usage();

        self.cpu_history.push(cpu_overall.round() as u64);
        if self.cpu_history.len() > HISTORY_LEN {
            let overflow = self.cpu_history.len() - HISTORY_LEN;
            self.cpu_history.drain(0..overflow);
        }

        let (net_rx_bps, net_tx_bps) = self
            .networks
            .iter()
            .filter(|(name, _)| name.as_str() != "lo")
            .fold((0u64, 0u64), |acc, (_, d)| {
                (acc.0 + d.received(), acc.1 + d.transmitted())
            });

        let (disk_read_bps, disk_write_bps) =
            self.sys.processes().values().fold((0u64, 0u64), |acc, p| {
                let du = p.disk_usage();
                (acc.0 + du.read_bytes, acc.1 + du.written_bytes)
            });

        let (disk_used, disk_total) = self
            .disks
            .iter()
            .find(|d| d.mount_point() == root_disk_path().as_path())
            .map(|d| (d.total_space() - d.available_space(), d.total_space()))
            .unwrap_or((0, 0));

        let cpu_temp = self
            .components
            .iter()
            .find(|c| {
                let l = c.label().to_lowercase();
                l.contains("package") || l.contains("tctl") || l.contains("cpu")
            })
            .or_else(|| self.components.iter().next())
            .map(|c| c.temperature())
            .flatten();

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
        top_procs.truncate(50);

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
            net_rx_bps,
            net_tx_bps,
            disk_read_bps,
            disk_write_bps,
            disk_used,
            disk_total,
            cpu_temp,
        }
    }
}

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

#[cfg(target_os = "windows")]
fn root_disk_path() -> std::path::PathBuf {
    let drive = std::env::var("SYSTEMDRIVE").unwrap_or_else(|_| "C:".to_string());
    std::path::PathBuf::from(format!("{drive}\\"))
}

#[cfg(not(target_os = "windows"))]
fn root_disk_path() -> std::path::PathBuf {
    std::path::PathBuf::from("/")
}
