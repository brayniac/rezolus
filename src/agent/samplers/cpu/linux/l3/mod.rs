/// This sampler is used to measure CPU L3 cache access and misses. It does this
/// by using two uncore PMUs for each L3 cache domain.
///
/// This requires that we identify each L3 cache domain but also identify the
/// correct raw perf events to use which are processor dependent.

const NAME: &str = "cpu_l3";

use crate::agent::*;

use perf_event::ReadFormat;
use perf_event::events::Event;
use tokio::sync::Mutex;
use walkdir::WalkDir;

use std::collections::HashSet;

mod stats;

use stats::*;

#[distributed_slice(SAMPLERS)]
fn init(config: Arc<Config>) -> SamplerResult {
    if !config.enabled(NAME) {
        return Ok(None);
    }

    let inner = CpuL3Inner::new()?;

    Ok(Some(Box::new(CpuL3 {
        inner: inner.into(),
    })))
}

struct CpuL3 {
    inner: Mutex<CpuL3Inner>,
}

#[async_trait]
impl Sampler for CpuL3 {
    async fn refresh(&self) {
        let mut inner = self.inner.lock().await;

        let _ = inner.refresh().await;
    }
}

struct CpuL3Inner {
    caches: Vec<L3Cache>,
}

impl CpuL3Inner {
    pub fn new() -> Result<Self, std::io::Error> {
        let caches = get_l3_caches()?;

        Ok(Self { caches })
    }

    pub async fn refresh(&mut self) -> Result<(), std::io::Error> {
        for cache in &mut self.caches {
            if let Ok(group) = cache.access.read_group() {
                if let (Some(access), Some(miss)) =
                    (group.get(&cache.access), group.get(&cache.miss))
                {
                    let access = access.value();
                    let miss = miss.value();

                    for cpu in &cache.shared_cores {
                        let _ = CPU_L3_ACCESS.set(*cpu, access);
                        let _ = CPU_L3_MISS.set(*cpu, miss);
                    }
                }
            }
        }

        Ok(())
    }
}

pub struct LowLevelEvent {
    event_type: u32,
    config: u64,
}

impl LowLevelEvent {
    pub fn new(event_type: u32, config: u64) -> Self {
        Self {
            event_type,
            config,
        }
    }
}

impl Event for LowLevelEvent {
    fn update_attrs(self, attr: &mut perf_event_open_sys::bindings::perf_event_attr) {
        attr.type_ = self.event_type;
        attr.config = self.config;
    }
}

/// A struct that contains the perf counters for each L3 cache as well as the
/// list of all CPUs in that L3 domain.
struct L3Cache {
    /// perf events for this cache
    access: perf_event::Counter,
    miss: perf_event::Counter,
    /// all cores which share this cache
    shared_cores: Vec<usize>,
}

impl L3Cache {
    pub fn new(shared_cores: Vec<usize>) -> Result<Self, ()> {
        let cpu = *shared_cores.first().expect("empty l3 domain");

        let (access_event, miss_event) = if let Some(events) = get_events() {
            events
        } else {
            return Err(());
        };

        if let Ok(mut access) = perf_event::Builder::new(access_event)
            .one_cpu(cpu)
            .any_pid()
            .exclude_hv(false)
            .exclude_kernel(false)
            .pinned(true)
            .read_format(
                ReadFormat::TOTAL_TIME_ENABLED | ReadFormat::TOTAL_TIME_RUNNING | ReadFormat::GROUP,
            )
            .build()
        {
            if let Ok(miss) = perf_event::Builder::new(miss_event)
                .one_cpu(cpu)
                .any_pid()
                .exclude_hv(false)
                .exclude_kernel(false)
                .build_with_group(&mut access)
            {
                match access.enable_group() {
                    Ok(_) => {
                        return Ok(L3Cache {
                            access,
                            miss,
                            shared_cores,
                        });
                    }
                    Err(e) => {
                        error!("failed to enable the perf group on CPU{cpu}: {e}");
                    }
                }
            }
        }

        Err(())
    }
}

fn l3_domains() -> Result<Vec<Vec<usize>>, std::io::Error> {
    let mut l3_domains = Vec::new();
    let mut processed = HashSet::new();

    // walk the cpu devices directory
    for entry in WalkDir::new("/sys/devices/system/cpu")
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let filename = path.file_name().and_then(|v| v.to_str()).unwrap_or("");

        // check if this is a cpu directory
        if filename.starts_with("cpu") && filename[3..].chars().all(char::is_numeric) {
            let cache_dir = path.join("cache");

            // look for the cache where level = 3
            if let Some(l3_index) = WalkDir::new(&cache_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .find(|entry| {
                    let index_path = entry.path();
                    index_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map_or(false, |name| {
                            name.starts_with("index")
                                && index_path.join("level").exists()
                                && std::fs::read_to_string(index_path.join("level"))
                                    .unwrap_or_default()
                                    .trim()
                                    == "3"
                        })
                })
            {
                let shared_cpu_list = l3_index.path().join("shared_cpu_list");

                // parse the shared cpu list
                if let Ok(shared_cpu_list) = std::fs::read_to_string(&shared_cpu_list) {
                    let shared_cores = parse_cpu_list(&shared_cpu_list);

                    // avoid duplicates
                    if !processed.contains(&shared_cores) {
                        processed.insert(shared_cores.clone());
                        l3_domains.push(shared_cores);
                    }
                }
            }
        }
    }

    Ok(l3_domains)
}

fn get_l3_caches() -> Result<Vec<L3Cache>, std::io::Error> {
    let mut l3_domains = l3_domains()?;

    let mut l3_caches = Vec::new();

    for l3_domain in l3_domains.drain(..) {
        if let Ok(l3_cache) = L3Cache::new(l3_domain) {
            l3_caches.push(l3_cache);
        }
    }

    Ok(l3_caches)
}

fn parse_cpu_list(list: &str) -> Vec<usize> {
    let mut cores = Vec::new();

    for range in list.trim().split(',') {
        if let Some((start, end)) = range.split_once('-') {
            // Range of cores
            if let (Ok(start_num), Ok(end_num)) = (start.parse::<usize>(), end.parse::<usize>()) {
                cores.extend(start_num..=end_num);
            }
        } else {
            // Single core
            if let Ok(core) = range.parse::<usize>() {
                cores.push(core);
            }
        }
    }

    cores.sort_unstable();
    cores.dedup();
    cores
}

fn get_events() -> Option<(LowLevelEvent, LowLevelEvent)> {
    let uarch = detect_microarchitecture();
    println!("detected uarch: {uarch}");

    match uarch {
        MicroArchitecture::ZenV1 => Some((LowLevelEvent::new(0xb, 0xFF04), LowLevelEvent::new(0xb, 0xFF04))),
        MicroArchitecture::ZenV2 => Some((LowLevelEvent::new(0xb, 0xFF04), LowLevelEvent::new(0xb, 0xFF04))),
        MicroArchitecture::ZenV3 => Some((LowLevelEvent::new(0xb, 0xFF04), LowLevelEvent::new(0xb, 0xFF04))),
        MicroArchitecture::ZenV4 => Some((LowLevelEvent::new(0xb, 0xFF04), LowLevelEvent::new(0xb, 0xFF04))),
        MicroArchitecture::ZenV5 => Some((LowLevelEvent::new(0xb, 0xFF04), LowLevelEvent::new(0xb, 0xFF04))),
        _ => None,
    }
}

use std::fmt;
use raw_cpuid::CpuId;

// Enum to represent different microarchitectures
#[derive(Debug, PartialEq)]
enum MicroArchitecture {
    // Intel Microarchitectures
    AlderLake,
    AlderLakeN,
    ArrowLake,
    Bonnell,
    Broadwell,
    BroadwellDE,
    BroadwellX,
    CascadeLakeX,
    ClearwaterForest,
    ElkhartLake,
    EmeraldRapids,
    Goldmont,
    GoldmontPlus,
    GrandRidge,
    GraniteRapids,
    Haswell,
    HaswellX,
    IceLake,
    IceLakeX,
    IvyBridge,
    IvyTown,
    JakeTown,
    KnightsLanding,
    LunarLake,
    MeteorLake,
    NehalemEP,
    NehalemEX,
    RocketLake,
    SandyBridge,
    SapphireRapids,
    SierraForest,
    Silvermont,
    Skylake,
    SkylakeX,
    SnowRidgeX,
    TigerLake,
    WestmereEPDP,
    WestmereEPSP,
    WestmereEX,

    // AMD Microarchitectures
    ZenV1,
    ZenV2,
    ZenV3,
    ZenV4,
    ZenV5,

    // Unknown Microarchitecture
    Unknown,
}

impl fmt::Display for MicroArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Intel Microarchitectures
            MicroArchitecture::AlderLake => write!(f, "alderlake"),
            MicroArchitecture::AlderLakeN => write!(f, "alderlaken"),
            MicroArchitecture::ArrowLake => write!(f, "arrowlake"),
            MicroArchitecture::Bonnell => write!(f, "bonnell"),
            MicroArchitecture::Broadwell => write!(f, "broadwell"),
            MicroArchitecture::BroadwellDE => write!(f, "broadwellde"),
            MicroArchitecture::BroadwellX => write!(f, "broadwellx"),
            MicroArchitecture::CascadeLakeX => write!(f, "cascadelakex"),
            MicroArchitecture::ClearwaterForest => write!(f, "clearwaterforest"),
            MicroArchitecture::ElkhartLake => write!(f, "elkhartlake"),
            MicroArchitecture::EmeraldRapids => write!(f, "emeraldrapids"),
            MicroArchitecture::Goldmont => write!(f, "goldmont"),
            MicroArchitecture::GoldmontPlus => write!(f, "goldmontplus"),
            MicroArchitecture::GrandRidge => write!(f, "grandridge"),
            MicroArchitecture::GraniteRapids => write!(f, "graniterapids"),
            MicroArchitecture::Haswell => write!(f, "haswell"),
            MicroArchitecture::HaswellX => write!(f, "haswellx"),
            MicroArchitecture::IceLake => write!(f, "icelake"),
            MicroArchitecture::IceLakeX => write!(f, "icelakex"),
            MicroArchitecture::IvyBridge => write!(f, "ivybridge"),
            MicroArchitecture::IvyTown => write!(f, "ivytown"),
            MicroArchitecture::JakeTown => write!(f, "jaketown"),
            MicroArchitecture::KnightsLanding => write!(f, "knightslanding"),
            MicroArchitecture::LunarLake => write!(f, "lunarlake"),
            MicroArchitecture::MeteorLake => write!(f, "meteorlake"),
            MicroArchitecture::NehalemEP => write!(f, "nehalemep"),
            MicroArchitecture::NehalemEX => write!(f, "nehalemex"),
            MicroArchitecture::RocketLake => write!(f, "rocketlake"),
            MicroArchitecture::SandyBridge => write!(f, "sandybridge"),
            MicroArchitecture::SapphireRapids => write!(f, "sapphirerapids"),
            MicroArchitecture::SierraForest => write!(f, "sierraforest"),
            MicroArchitecture::Silvermont => write!(f, "silvermont"),
            MicroArchitecture::Skylake => write!(f, "skylake"),
            MicroArchitecture::SkylakeX => write!(f, "skylakex"),
            MicroArchitecture::SnowRidgeX => write!(f, "snowridgex"),
            MicroArchitecture::TigerLake => write!(f, "tigerlake"),
            MicroArchitecture::WestmereEPDP => write!(f, "westmereep-dp"),
            MicroArchitecture::WestmereEPSP => write!(f, "westmereep-sp"),
            MicroArchitecture::WestmereEX => write!(f, "westmereex"),

            // AMD Microarchitectures
            MicroArchitecture::ZenV1 => write!(f, "amdzen1"),
            MicroArchitecture::ZenV2 => write!(f, "amdzen2"),
            MicroArchitecture::ZenV3 => write!(f, "amdzen3"),
            MicroArchitecture::ZenV4 => write!(f, "amdzen4"),
            MicroArchitecture::ZenV5 => write!(f, "amdzen5"),

            // Unknown
            MicroArchitecture::Unknown => write!(f, "unknown"),
        }
    }
}

// Function to detect microarchitecture using CPUID
fn detect_microarchitecture() -> MicroArchitecture {
    let cpuid = CpuId::new();

    // Get vendor string and feature information
    let vendor_info = if let Some(vendor_info) = cpuid.get_vendor_info() {
        vendor_info
    } else {
        return MicroArchitecture::Unknown;
    };

    let feature_info = if let Some(feature_info) = cpuid.get_feature_info() {
        feature_info
    } else {
        return MicroArchitecture::Unknown;
    };

    // Family and model are important for microarchitecture detection
    let family = feature_info.family_id();
    let model = feature_info.model_id();
    let extended_model = feature_info.extended_model_id();
    let full_model = (extended_model << 4) | model;

    // Vendor-specific detection
    let result = match vendor_info.as_str() {
        "GenuineIntel" => detect_intel_microarchitecture(family, full_model),
        "AuthenticAMD" => detect_amd_microarchitecture(family, full_model),
        _ => MicroArchitecture::Unknown,
    };

    if result == MicroArchitecture::Unknown {
        println!("family: {family} model: {full_model}");
    }

    result
}

// Detect Intel Microarchitecture
fn detect_intel_microarchitecture(family: u8, model: u8) -> MicroArchitecture {
    // Ensure we're dealing with Intel's family 6 processors
    if family != 6 {
        return MicroArchitecture::Unknown;
    }

    match model {
        // // Alder Lake
        // 0x97 | 0x9A | 0xB7 | 0xBA | 0xBF => MicroArchitecture::AlderLake,
        // 0xBE => MicroArchitecture::AlderLakeN,

        // // Arrow Lake
        // 0xC5 | 0xC6 => MicroArchitecture::ArrowLake,

        // // Bonnell
        // 0x1C | 0x26 | 0x27 | 0x35 | 0x36 => MicroArchitecture::Bonnell,

        // // Broadwell
        // 0x3D | 0x47 => MicroArchitecture::Broadwell,
        // 0x56 => MicroArchitecture::BroadwellDE,
        // 0x4F => MicroArchitecture::BroadwellX,

        // // Cascade Lake X
        // 0x55 if (model & 0xF) >= 5 => MicroArchitecture::CascadeLakeX,

        // // Other specific models
        // 0xDD => MicroArchitecture::ClearwaterForest,
        // 0x9C | 0x96 => MicroArchitecture::ElkhartLake,
        // 0xCF => MicroArchitecture::EmeraldRapids,
        // 0x5C | 0x5F => MicroArchitecture::Goldmont,
        // 0x7A => MicroArchitecture::GoldmontPlus,
        // 0xB6 => MicroArchitecture::GrandRidge,
        // 0xAD | 0xAE | 0xA6 => MicroArchitecture::GraniteRapids,

        // // Haswell
        // 0x3C | 0x45 | 0x46 => MicroArchitecture::Haswell,
        // 0x3F => MicroArchitecture::HaswellX,

        // // Ice Lake
        // 0x7D | 0x7E => MicroArchitecture::IceLake,
        // 0x6A | 0x6C => MicroArchitecture::IceLakeX,

        // // Ivy Bridge
        // 0x3A => MicroArchitecture::IvyBridge,
        // 0x3E => MicroArchitecture::IvyTown,
        // 0x2D => MicroArchitecture::JakeTown,

        // // Knights Landing
        // 0x57 | 0x85 => MicroArchitecture::KnightsLanding,

        // // Lunar Lake
        // 0xBD => MicroArchitecture::LunarLake,

        // // Meteor Lake
        // 0xAA | 0xAC | 0xB5 => MicroArchitecture::MeteorLake,

        // // Nehalem
        // 0x1A | 0x1E | 0x1F => MicroArchitecture::NehalemEP,
        // 0x2E => MicroArchitecture::NehalemEX,

        // // Rocket Lake
        // 0xA7 => MicroArchitecture::RocketLake,

        // // Sandy Bridge
        // 0x2A => MicroArchitecture::SandyBridge,

        // // Sapphire Rapids
        // 0x8F => MicroArchitecture::SapphireRapids,

        // // Sierra Forest
        // 0xAF => MicroArchitecture::SierraForest,

        // // Silvermont
        // 0x37 | 0x4A | 0x4C | 0x4D | 0x5A => MicroArchitecture::Silvermont,

        // // Skylake
        // 0x4E | 0x5E | 0x8E | 0x9E | 0xA5 | 0xA6 => MicroArchitecture::Skylake,
        // 0x55 if (model & 0xF) <= 4 => MicroArchitecture::SkylakeX,

        // // Snow Ridge X
        // 0x86 => MicroArchitecture::SnowRidgeX,

        // // Tiger Lake
        // 0x8C | 0x8D => MicroArchitecture::TigerLake,

        // // Westmere
        // 0x2C => MicroArchitecture::WestmereEPDP,
        // 0x25 => MicroArchitecture::WestmereEPSP,
        // 0x2F => MicroArchitecture::WestmereEX,

        _ => MicroArchitecture::Unknown,
    }
}

// Detect AMD Microarchitecture
fn detect_amd_microarchitecture(family: u8, model: u8) -> MicroArchitecture {
    match family {
        // Zen V1 (Ryzen 1000/2000 series)
        23 if model >= 0x00 && model <= 0x2F => MicroArchitecture::ZenV1,

        // Zen V2 (Ryzen 3000 series)
        23 if model >= 0x30 && model <= 0x3F => MicroArchitecture::ZenV2,

        // Zen V3 (Ryzen 5000 series)
        25 if model >= 0x00 && model <= 0x2F => MicroArchitecture::ZenV3,

        // Zen V4 (Ryzen 7000 series)
        25 if model >= 0x30 && model <= 0x3F => MicroArchitecture::ZenV4,

        // Zen V5 (Ryzen 8000 series)
        26 => MicroArchitecture::ZenV5,

        _ => MicroArchitecture::Unknown,
    }
}
