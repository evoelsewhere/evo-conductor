use std::sync::Mutex;
use std::time::Instant;

use chrono::Utc;
use conductor_domain::{DashboardHostMetrics, DashboardHostMetricsScope};
use sysinfo::{System, MINIMUM_CPU_UPDATE_INTERVAL};

/// Narrow sampling boundary for Conductor-host metrics.
///
/// Implementations must never shell out. Device metrics from EvoFlux clients
/// are intentionally outside this boundary.
pub trait HostMetricsProvider: Send + Sync + 'static {
    fn sample(&self) -> DashboardHostMetrics;
}

pub struct SystemHostMetricsProvider {
    state: Mutex<SystemMetricsState>,
}

struct SystemMetricsState {
    system: System,
    cpu_baseline_at: Instant,
    cpu_ready: bool,
}

impl Default for SystemHostMetricsProvider {
    fn default() -> Self {
        let mut system = System::new();
        system.refresh_memory();
        // CPU usage is a delta. This first refresh establishes a baseline;
        // callers see null until the minimum sampling interval has elapsed.
        system.refresh_cpu_usage();
        Self {
            state: Mutex::new(SystemMetricsState {
                system,
                cpu_baseline_at: Instant::now(),
                cpu_ready: false,
            }),
        }
    }
}

impl HostMetricsProvider for SystemHostMetricsProvider {
    fn sample(&self) -> DashboardHostMetrics {
        self.sample_at(Utc::now(), Instant::now())
    }
}

impl SystemHostMetricsProvider {
    fn sample_at(&self, sampled_at: chrono::DateTime<Utc>, now: Instant) -> DashboardHostMetrics {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.system.refresh_memory();

        if now.saturating_duration_since(state.cpu_baseline_at) >= MINIMUM_CPU_UPDATE_INTERVAL {
            state.system.refresh_cpu_usage();
            state.cpu_baseline_at = now;
            state.cpu_ready = true;
        }

        let cpu_usage_percent = state
            .cpu_ready
            .then(|| f64::from(state.system.global_cpu_usage()).clamp(0.0, 100.0));
        let memory_total_bytes = state.system.total_memory();
        let (memory_used_bytes, memory_total_bytes) = if memory_total_bytes > 0 {
            (
                Some(state.system.used_memory().min(memory_total_bytes)),
                Some(memory_total_bytes),
            )
        } else {
            (None, None)
        };

        DashboardHostMetrics {
            scope: DashboardHostMetricsScope::ConductorHost,
            sampled_at,
            cpu_usage_percent,
            memory_used_bytes,
            memory_total_bytes,
            // sysinfo does not provide a portable GPU API. Unsupported is
            // represented as null, never a fabricated zero-utilization GPU.
            gpu_usage_percent: None,
            vram_used_bytes: None,
            vram_total_bytes: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;

    #[test]
    fn first_host_sample_keeps_warming_cpu_and_unsupported_gpu_nullable() {
        let baseline = Instant::now();
        let mut system = System::new();
        system.refresh_memory();
        system.refresh_cpu_usage();
        let provider = SystemHostMetricsProvider {
            state: Mutex::new(SystemMetricsState {
                system,
                cpu_baseline_at: baseline,
                cpu_ready: false,
            }),
        };
        let sampled_at = DateTime::parse_from_rfc3339("2026-08-18T05:06:07Z")
            .expect("fixed timestamp")
            .with_timezone(&Utc);

        let metrics = provider.sample_at(sampled_at, baseline);

        assert_eq!(metrics.scope, DashboardHostMetricsScope::ConductorHost);
        assert_eq!(metrics.sampled_at, sampled_at);
        assert_eq!(metrics.cpu_usage_percent, None);
        assert_eq!(metrics.gpu_usage_percent, None);
        assert_eq!(metrics.vram_used_bytes, None);
        assert_eq!(metrics.vram_total_bytes, None);
    }
}
