use crate::*;

bpfhistogram!(
    SCHEDULER_RUNQUEUE_LATENCY,
    "scheduler/runqueue/latency",
    "distribution of task wait times in the runqueue",
    42
);
bpfhistogram!(
    SCHEDULER_RUNNING,
    "scheduler/running",
    "distribution of task on-cpu time",
    42
);
counter!(
    SCHEDULER_IVCSW,
    "scheduler/context_switch/involuntary",
    "count of involuntary context switches"
);
