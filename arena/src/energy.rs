//! Energy: what a unit of work actually cost, and how we know.
//!
//! The arena's objective is intelligence per second per watt. A watt is a joule
//! per second, so that ratio is intelligence per **joule** — and a joule figure
//! is only as good as the instrument behind it. So every reading carries its
//! [`Basis`], and the report says which figures were measured and which were
//! estimated rather than printing one number with the difference erased.
//!
//! * [`Nvml`] reads each GPU's cumulative energy counter
//!   (`nvmlDeviceGetTotalEnergyConsumption`, millijoules since driver load).
//!   That is a hardware counter, not a sampled power reading integrated in
//!   software, so a span's energy is an exact difference. **Measured.**
//! * [`WallClock`] multiplies elapsed time by a stated wattage. It exists
//!   because the CPU has no counter this process can read on Windows without a
//!   privileged driver. **Estimated**, and it says so, with the wattage it
//!   assumed.
//!
//! A meter that cannot read returns [`Basis::Unavailable`], never zero. Zero
//! joules would read as *free*, and a search would pour its budget into
//! whatever was unmeasured.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// How a joule figure was obtained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum Basis {
    /// Read from a hardware energy counter.
    Measured,
    /// Computed from a model; `how` states the model.
    Estimated { how: String },
    /// No reading; `why` says what was missing.
    Unavailable { why: String },
}

/// One meter's reading over one span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub meter: String,
    pub joules: Option<f64>,
    pub basis: Basis,
}

/// Something that can say how much energy has been used since it started.
pub trait Meter {
    fn name(&self) -> String;
    /// Begin a span.
    fn start(&mut self);
    /// End the span and report it.
    fn stop(&mut self) -> Reading;
}

// ── NVML ─────────────────────────────────────────────────────────────────

type NvmlReturn = i32;
type DeviceHandle = *mut std::ffi::c_void;
const NVML_SUCCESS: NvmlReturn = 0;

/// Every visible NVIDIA GPU's energy counter, summed.
pub struct Nvml {
    // Field order is drop order: the symbols are copied out of the library, so
    // the library must outlive every call, which owning it here guarantees.
    energy: unsafe extern "C" fn(DeviceHandle, *mut u64) -> NvmlReturn,
    shutdown: unsafe extern "C" fn() -> NvmlReturn,
    devices: Vec<DeviceHandle>,
    /// NVML indices of `devices`, for the meter's name.
    indices: Vec<u32>,
    at_start: Option<u64>,
    _lib: libloading::Library,
}

impl Nvml {
    /// Load NVML and open every device. `Err` explains why not — no driver,
    /// no devices, or a counter the hardware does not provide (pre-Volta).
    pub fn open() -> Result<Nvml, String> {
        Nvml::open_devices(None)
    }

    /// Open only the listed NVML device indices, or every device for `None`.
    ///
    /// Summing every GPU was right on an otherwise idle machine and wrong the
    /// first time it was not: a 25M-parameter run pinned to GPU 1 reported
    /// 585 kJ by round 25 while GPU 0 ran someone else's workload at 100%.
    /// The joules a learner is charged must be the joules of the device it
    /// runs on. Indices are NVML's, which need not match CUDA's ordering, so
    /// they are given explicitly rather than guessed from
    /// `CUDA_VISIBLE_DEVICES`.
    pub fn open_devices(only: Option<&[u32]>) -> Result<Nvml, String> {
        let names: &[&str] = if cfg!(windows) {
            &["nvml.dll"]
        } else {
            &["libnvidia-ml.so.1", "libnvidia-ml.so"]
        };
        // SAFETY: loading a system library by name runs its initialisers; NVML's
        // are the vendor's documented entry path.
        let lib = names
            .iter()
            .find_map(|n| unsafe { libloading::Library::new(n) }.ok())
            .ok_or_else(|| format!("NVML not found (tried {})", names.join(", ")))?;
        // SAFETY: each symbol is declared with the signature in NVML's public
        // header (nvml.h); the pointers are only called while `lib` is alive.
        unsafe {
            let init = *lib
                .get::<unsafe extern "C" fn() -> NvmlReturn>(b"nvmlInit_v2\0")
                .map_err(|e| format!("nvmlInit_v2: {e}"))?;
            let count = *lib
                .get::<unsafe extern "C" fn(*mut u32) -> NvmlReturn>(b"nvmlDeviceGetCount_v2\0")
                .map_err(|e| format!("nvmlDeviceGetCount_v2: {e}"))?;
            let handle = *lib
                .get::<unsafe extern "C" fn(u32, *mut DeviceHandle) -> NvmlReturn>(
                    b"nvmlDeviceGetHandleByIndex_v2\0",
                )
                .map_err(|e| format!("nvmlDeviceGetHandleByIndex_v2: {e}"))?;
            let energy = *lib
                .get::<unsafe extern "C" fn(DeviceHandle, *mut u64) -> NvmlReturn>(
                    b"nvmlDeviceGetTotalEnergyConsumption\0",
                )
                .map_err(|e| format!("nvmlDeviceGetTotalEnergyConsumption: {e}"))?;
            let shutdown = *lib
                .get::<unsafe extern "C" fn() -> NvmlReturn>(b"nvmlShutdown\0")
                .map_err(|e| format!("nvmlShutdown: {e}"))?;
            if init() != NVML_SUCCESS {
                return Err("nvmlInit_v2 failed".into());
            }
            let mut n = 0u32;
            if count(&mut n) != NVML_SUCCESS || n == 0 {
                shutdown();
                return Err("NVML reports no devices".into());
            }
            let mut devices = Vec::new();
            let mut indices = Vec::new();
            for i in 0..n {
                if let Some(want) = only {
                    if !want.contains(&i) {
                        continue;
                    }
                }
                let mut h: DeviceHandle = std::ptr::null_mut();
                if handle(i, &mut h) == NVML_SUCCESS {
                    devices.push(h);
                    indices.push(i);
                }
            }
            if let Some(want) = only {
                if let Some(missing) = want.iter().find(|w| !indices.contains(w)) {
                    shutdown();
                    return Err(format!("NVML device {missing} does not exist (found {n})"));
                }
            }
            let nvml = Nvml { energy, shutdown, devices, indices, at_start: None, _lib: lib };
            nvml.total_mj().map_err(|e| format!("energy counter unreadable: {e}"))?;
            Ok(nvml)
        }
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    fn total_mj(&self) -> Result<u64, String> {
        let mut sum = 0u64;
        for (i, &d) in self.devices.iter().enumerate() {
            let mut mj = 0u64;
            // SAFETY: `d` came from nvmlDeviceGetHandleByIndex_v2 on this
            // library instance, which is still loaded and initialised.
            let rc = unsafe { (self.energy)(d, &mut mj) };
            if rc != NVML_SUCCESS {
                return Err(format!("device {i}: nvml return {rc}"));
            }
            sum += mj;
        }
        Ok(sum)
    }
}

impl Drop for Nvml {
    fn drop(&mut self) {
        // SAFETY: paired with the successful nvmlInit_v2 in `open`.
        unsafe {
            (self.shutdown)();
        }
    }
}

impl Meter for Nvml {
    fn name(&self) -> String {
        let ids: Vec<String> = self.indices.iter().map(|i| i.to_string()).collect();
        format!("nvml:gpu[{}]", ids.join(","))
    }

    fn start(&mut self) {
        self.at_start = self.total_mj().ok();
    }

    fn stop(&mut self) -> Reading {
        let now = self.total_mj();
        match (self.at_start.take(), now) {
            (Some(a), Ok(b)) => Reading {
                meter: self.name(),
                joules: Some(b.saturating_sub(a) as f64 / 1000.0),
                basis: Basis::Measured,
            },
            (_, Err(e)) => Reading {
                meter: self.name(),
                joules: None,
                basis: Basis::Unavailable { why: e },
            },
            (None, Ok(_)) => Reading {
                meter: self.name(),
                joules: None,
                basis: Basis::Unavailable { why: "stop without a successful start".into() },
            },
        }
    }
}

// ── Wall-clock estimate ──────────────────────────────────────────────────

/// Elapsed time multiplied by an assumed wattage.
pub struct WallClock {
    pub label: String,
    pub watts: f64,
    started: Option<Instant>,
}

impl WallClock {
    pub fn new(label: impl Into<String>, watts: f64) -> Self {
        WallClock { label: label.into(), watts, started: None }
    }
}

impl Meter for WallClock {
    fn name(&self) -> String {
        self.label.clone()
    }

    fn start(&mut self) {
        self.started = Some(Instant::now());
    }

    fn stop(&mut self) -> Reading {
        match self.started.take() {
            Some(t) => Reading {
                meter: self.name(),
                joules: Some(t.elapsed().as_secs_f64() * self.watts),
                basis: Basis::Estimated {
                    how: format!("wall-clock seconds × {} W assumed", self.watts),
                },
            },
            None => Reading {
                meter: self.name(),
                joules: None,
                basis: Basis::Unavailable { why: "stop without start".into() },
            },
        }
    }
}

/// A span across several meters at once.
pub struct Span {
    started: Instant,
}

impl Span {
    pub fn start(meters: &mut [Box<dyn Meter>]) -> Span {
        for m in meters.iter_mut() {
            m.start();
        }
        Span { started: Instant::now() }
    }

    /// Stop every meter; returns (seconds, readings).
    pub fn stop(self, meters: &mut [Box<dyn Meter>]) -> (f64, Vec<Reading>) {
        let secs = self.started.elapsed().as_secs_f64();
        (secs, meters.iter_mut().map(|m| m.stop()).collect())
    }
}

/// Total joules over the readings that have a figure, and whether every one
/// of them was measured. `None` when no meter produced a figure at all.
pub fn total(readings: &[Reading]) -> Option<(f64, bool)> {
    let with: Vec<&Reading> = readings.iter().filter(|r| r.joules.is_some()).collect();
    if with.is_empty() {
        return None;
    }
    let j = with.iter().map(|r| r.joules.unwrap_or(0.0)).sum();
    let all_measured = readings.iter().all(|r| r.basis == Basis::Measured);
    Some((j, all_measured))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wall_clock_is_labelled_an_estimate() {
        let mut m = WallClock::new("cpu", 65.0);
        m.start();
        let r = m.stop();
        assert!(matches!(r.basis, Basis::Estimated { .. }));
        assert!(r.joules.unwrap() >= 0.0);
    }

    #[test]
    fn an_unstarted_meter_is_unavailable_not_zero() {
        let mut m = WallClock::new("cpu", 65.0);
        let r = m.stop();
        assert_eq!(r.joules, None);
        assert!(matches!(r.basis, Basis::Unavailable { .. }));
    }

    #[test]
    fn total_says_whether_everything_was_measured() {
        let measured = Reading { meter: "a".into(), joules: Some(2.0), basis: Basis::Measured };
        let est = Reading {
            meter: "b".into(),
            joules: Some(3.0),
            basis: Basis::Estimated { how: "x".into() },
        };
        assert_eq!(total(&[measured.clone()]), Some((2.0, true)));
        assert_eq!(total(&[measured, est]), Some((5.0, false)));
        assert_eq!(total(&[]), None);
    }

    #[test]
    fn nvml_meters_only_the_devices_it_is_given() {
        match Nvml::open_devices(Some(&[0])) {
            Ok(n) => {
                assert_eq!(n.device_count(), 1);
                assert_eq!(n.name(), "nvml:gpu[0]");
                // A device that does not exist is refused, not silently dropped:
                // metering nothing would read as free.
                assert!(Nvml::open_devices(Some(&[99])).is_err());
            }
            Err(why) => assert!(!why.is_empty()),
        }
    }

    #[test]
    fn nvml_either_measures_or_explains() {
        // Hardware-dependent by nature: on a machine with an NVIDIA driver the
        // counter must read and be monotone; without one, `open` must say why.
        match Nvml::open() {
            Ok(mut n) => {
                n.start();
                let r = n.stop();
                assert_eq!(r.basis, Basis::Measured, "{r:?}");
                assert!(r.joules.unwrap() >= 0.0);
            }
            Err(why) => assert!(!why.is_empty()),
        }
    }
}
