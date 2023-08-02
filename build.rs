fn main() {
    #[cfg(all(feature = "bpf", target_os = "linux"))]
    bpf::generate();
}

#[cfg(all(feature = "bpf", target_os = "linux"))]
mod bpf {
    use libbpf_cargo::SkeletonBuilder;

    // `SOURCES` lists all BPF programs and the sampler that contains them.
    // Each entry `(sampler, program)` maps to a unique path in the `samplers`
    // directory.
    const SOURCES: &'static [(&str, &str)] = &[
        ("block_io", "latency"),
        ("scheduler", "runqueue"),
        ("syscall", "latency"),
        ("tcp", "packet_latency"),
        ("tcp", "receive"),
        ("tcp", "retransmit"),
        ("tcp", "traffic"),
    ];

    pub fn generate() {
        let out_dir = std::env::var("OUT_DIR").unwrap();

        for (sampler, prog) in SOURCES {
            let src = format!("src/samplers/{sampler}/{prog}/mod.bpf.c");
            let tgt = format!("{out_dir}/{sampler}_{prog}.bpf.rs");
            let skel_builder = SkeletonBuilder::new();

            #[cfg(target_arch = "x86_64")]
            let skel_builder = skel_builder.clang_args("-I src/common/bpf/x86_64");

            #[cfg(target_arch = "aarch64")]
            let skel_builder = skel_builder.clang_args("-I src/common/bpf/aarch64");

            skel_builder
                .source(&src)
                .build_and_generate(&tgt)
                .unwrap();
            println!("cargo:rerun-if-changed={src}");
        }

        println!("cargo:rerun-if-changed=src/common/bpf/histogram.h");
        println!("cargo:rerun-if-changed=src/common/bpf/vmlinux.h");
    }
}
