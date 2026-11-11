use std::time::Instant;

use anyhow::Context;
use ash::vk;
use parking_lot::Mutex;

use crate::gpu::command_buffer::CommandBuffer;

struct PendingProfilerSpan {
    name: &'static str,
    start_time: Instant,
    end_time: Instant,
    start_query_index: u32,
    end_query_index: u32,
    depth: usize,
}

struct ProfilerSpan {
    name: &'static str,
    cpu_duration_us: f64,
    gpu_duration_us: f64,
    depth: usize,
}

#[derive(Default)]
struct FrameProfilerState {
    query_index: u32,
    scope_depth: usize,

    pending_spans: Vec<PendingProfilerSpan>,
    resolved_frames: Vec<Vec<ProfilerSpan>>,
}

pub struct FrameProfiler {
    device: ash::Device,
    query_pool: vk::QueryPool,

    timestamp_period: f32,

    state: Mutex<FrameProfilerState>,
}

impl FrameProfiler {
    pub const MAX_PROFILER_SPANS: u32 = 256;
    pub const MAX_PROFILER_QUERIES: u32 = Self::MAX_PROFILER_SPANS * 2;
    /// The maximum number of frames to keep for averaging results.
    pub const MAX_RESOLVED_FRAMES: usize = 60;

    pub fn new(device: &ash::Device, timestamp_period: f32) -> anyhow::Result<Self> {
        let query_pool = unsafe {
            device.create_query_pool(
                &vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    .query_count(Self::MAX_PROFILER_QUERIES),
                None,
            )
        }
        .context("creating profiler query pool")?;

        Ok(Self {
            device: device.clone(),
            query_pool,
            timestamp_period,
            state: Mutex::new(FrameProfilerState::default()),
        })
    }

    pub(super) fn begin_frame(&self) {
        self.resolve_pending_queries();

        let mut state = self.state.lock();
        state.query_index = 0;
        state.pending_spans.clear();
        state.scope_depth = 0;

        unsafe {
            self.device
                .reset_query_pool(self.query_pool, 0, Self::MAX_PROFILER_QUERIES);
        }
    }

    fn resolve_pending_queries(&self) {
        let mut state = self.state.lock();

        while state.resolved_frames.len() >= Self::MAX_RESOLVED_FRAMES {
            state.resolved_frames.remove(0);
        }

        let mut resolved_spans = Vec::with_capacity(state.pending_spans.len());
        for span in &state.pending_spans {
            let gpu_start_ticks = self.read_timestamp(span.start_query_index);
            let gpu_end_ticks = self.read_timestamp(span.end_query_index);

            let gpu_duration_ticks = gpu_end_ticks.saturating_sub(gpu_start_ticks);
            let gpu_duration_us = gpu_duration_ticks as f64 * self.timestamp_period as f64 / 1000.0;

            let cpu_duration_us =
                span.end_time.duration_since(span.start_time).as_secs_f64() * 1_000_000.0;

            resolved_spans.push(ProfilerSpan {
                name: span.name,
                cpu_duration_us,
                gpu_duration_us,
                depth: span.depth,
            });
        }

        state.resolved_frames.push(resolved_spans);
    }

    fn read_timestamp(&self, query: u32) -> u64 {
        let mut data = [0u64; 1];
        unsafe {
            self.device
                .get_query_pool_results(
                    self.query_pool,
                    query,
                    &mut data,
                    vk::QueryResultFlags::TYPE_64,
                )
                .ok();
        }
        data[0]
    }

    pub fn get_results_string(&self) -> String {
        let state = self.state.lock();
        if state.resolved_frames.is_empty() {
            return "No profiling data available yet".to_string();
        }

        let mut results: Vec<ProfilerSpan> = Vec::new();
        for scope in state
            .resolved_frames
            .last()
            .expect("resolved_frames cannot be empty")
        {
            if results
                .iter()
                .any(|r| r.name == scope.name && r.depth == scope.depth)
            {
                continue;
            }

            let mut total_cpu = 0.0;
            let mut total_gpu = 0.0;
            let mut count = 0;
            for frame in state.resolved_frames.iter().rev() {
                let mut frame_cpu = 0.0;
                let mut frame_gpu = 0.0;
                let mut found = false;
                for s in frame
                    .iter()
                    .filter(|s| s.name == scope.name && s.depth == scope.depth)
                {
                    frame_cpu += s.cpu_duration_us;
                    frame_gpu += s.gpu_duration_us;
                    found = true;
                }
                if found {
                    total_cpu += frame_cpu;
                    total_gpu += frame_gpu;
                    count += 1;
                }
            }

            results.push(ProfilerSpan {
                name: scope.name,
                depth: scope.depth,
                cpu_duration_us: total_cpu / count as f64,
                gpu_duration_us: total_gpu / count as f64,
            });
        }

        let mut output = String::new();
        output.push_str(&format!(
            "{:<30} {:>12} {:>12}\n",
            "Scope", "CPU (ms)", "GPU (ms)"
        ));
        output.push_str(&"-".repeat(56));
        output.push('\n');

        let mut longest_duration_cpu = 0f64;
        let mut longest_duration_gpu = 0f64;
        for span in results {
            let scope_name = if span.depth > 0 {
                format!("{}{}", "  ".repeat(span.depth), span.name)
            } else {
                span.name.to_string()
            };
            output.push_str(&format!(
                "{:<30} {:>12.3} {:>12.3}\n",
                scope_name,
                span.cpu_duration_us / 1000.0,
                span.gpu_duration_us / 1000.0
            ));
            longest_duration_cpu = longest_duration_cpu.max(span.cpu_duration_us);
            longest_duration_gpu = longest_duration_gpu.max(span.gpu_duration_us);
        }

        output.push_str(&"-".repeat(56));
        output.push('\n');
        output.push_str(&format!(
            "{:<30} {:>12.1} {:>12.1}\n",
            "Potential FPS",
            if longest_duration_cpu > 0.0 {
                1_000_000.0 / longest_duration_cpu
            } else {
                f64::INFINITY
            },
            if longest_duration_gpu > 0.0 {
                1_000_000.0 / longest_duration_gpu
            } else {
                f64::INFINITY
            }
        ));

        output
    }

    pub fn scope<'a>(&'a self, cmd: &CommandBuffer, name: &'static str) -> ScopeGuard<'a> {
        let mut state = self.state.lock();

        if state.pending_spans.len() as u32 >= Self::MAX_PROFILER_SPANS {
            warn!("Exceeded maximum number of profiler spans");
            return ScopeGuard {
                profiler: self,
                cmd: cmd.clone(),
                pending_span_index: usize::MAX,
            };
        }

        let start_query_index = state.query_index;
        let end_query_index = state.query_index + 1;
        state.query_index += 2;
        let pending_span_index = state.pending_spans.len();
        let depth = state.scope_depth;
        state.scope_depth += 1;
        state.pending_spans.push(PendingProfilerSpan {
            name,
            start_time: Instant::now(),
            end_time: Instant::now(),
            start_query_index,
            end_query_index,
            depth,
        });

        unsafe {
            self.device.cmd_write_timestamp(
                **cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                self.query_pool,
                start_query_index,
            );
        }

        ScopeGuard {
            profiler: self,
            cmd: cmd.clone(),
            pending_span_index,
        }
    }
}

impl Drop for FrameProfiler {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_query_pool(self.query_pool, None);
        }
    }
}

pub struct ScopeGuard<'a> {
    profiler: &'a FrameProfiler,
    cmd: CommandBuffer,
    pending_span_index: usize,
}

impl Drop for ScopeGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.profiler.state.lock();

        let Some(pending_span) = state.pending_spans.get_mut(self.pending_span_index) else {
            warn!("Invalid profiler scope guard index");
            return;
        };

        pending_span.end_time = Instant::now();
        unsafe {
            self.profiler.device.cmd_write_timestamp(
                *self.cmd,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.profiler.query_pool,
                pending_span.end_query_index,
            );
        }

        state.scope_depth = state.scope_depth.saturating_sub(1);
    }
}
