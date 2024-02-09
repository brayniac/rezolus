use crate::*;

bpfhistogram!(
    BLOCKIO_LATENCY,
    "blockio/latency",
    "distribution of block IO latencies",
    42
);
bpfhistogram!(
    BLOCKIO_SIZE,
    "blockio/size",
    "distribution of block IO sizes",
    42
);
