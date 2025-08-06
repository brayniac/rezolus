// Chart metadata enrichment
// Adds context and descriptions to charts for better search and understanding

export class ChartEnricher {
    constructor() {
        // Define section context and common patterns
        this.sectionContext = {
            cpu: {
                prefix: 'CPU',
                description: 'Processor utilization and performance metrics',
                keywords: ['processor', 'cores', 'usage', 'utilization', 'frequency']
            },
            network: {
                prefix: 'Network',
                description: 'Network traffic and connection metrics',
                keywords: ['traffic', 'packets', 'bytes', 'connections', 'bandwidth']
            },
            blockio: {
                prefix: 'Disk I/O',
                description: 'Storage device input/output operations',
                keywords: ['disk', 'storage', 'read', 'write', 'io', 'operations']
            },
            scheduler: {
                prefix: 'Scheduler',
                description: 'Process scheduling and context switching metrics',
                keywords: ['scheduling', 'processes', 'threads', 'context', 'switches']
            },
            syscall: {
                prefix: 'System Call',
                description: 'Kernel system call metrics',
                keywords: ['kernel', 'syscall', 'system', 'calls']
            },
            softirq: {
                prefix: 'Soft IRQ',
                description: 'Software interrupt handling metrics',
                keywords: ['interrupt', 'irq', 'software', 'handlers']
            },
            memory: {
                prefix: 'Memory',
                description: 'System memory usage and allocation',
                keywords: ['ram', 'memory', 'heap', 'cache', 'buffer', 'swap']
            },
            cgroups: {
                prefix: 'Container',
                description: 'Container and cgroup resource metrics',
                keywords: ['container', 'cgroup', 'docker', 'kubernetes', 'pod']
            },
            overview: {
                prefix: 'System',
                description: 'System-wide overview metrics',
                keywords: ['system', 'overall', 'general', 'summary']
            },
            rezolus: {
                prefix: 'Rezolus',
                description: 'Monitoring collector internal metrics',
                keywords: ['collector', 'monitoring', 'rezolus', 'internal']
            }
        };

        // Common metric patterns and their descriptions
        this.metricPatterns = {
            // CPU patterns
            'busy %': 'Percentage of time CPU is busy processing tasks',
            'idle %': 'Percentage of time CPU is idle',
            'user %': 'CPU time spent in user mode',
            'system %': 'CPU time spent in kernel/system mode',
            'iowait %': 'CPU time waiting for I/O operations',
            'frequency': 'Current CPU frequency in MHz or GHz',
            
            // Network patterns
            'rx': 'Received/incoming network traffic',
            'tx': 'Transmitted/outgoing network traffic',
            'bytes/s': 'Network throughput in bytes per second',
            'packets/s': 'Network packet rate per second',
            'errors': 'Network transmission or reception errors',
            'dropped': 'Dropped network packets',
            
            // Disk patterns
            'read': 'Disk read operations or throughput',
            'write': 'Disk write operations or throughput',
            'operations/s': 'Disk operations per second (IOPS)',
            'bytes/s': 'Disk throughput in bytes per second',
            'latency': 'Disk operation latency in milliseconds',
            'utilization': 'Disk utilization percentage',
            
            // Memory patterns
            'used': 'Memory currently in use',
            'free': 'Available free memory',
            'cached': 'Memory used for caching',
            'buffers': 'Memory used for buffers',
            'swap': 'Swap space usage',
            
            // System patterns
            'load average': 'System load average (1, 5, or 15 minute)',
            'processes': 'Number of processes',
            'threads': 'Number of threads',
            'context switches': 'Rate of context switches between processes',
            'uptime': 'System uptime duration'
        };
    }

    // Enrich a chart with full context and description
    enrichChart(chart, section, group) {
        const enriched = { ...chart };
        const sectionInfo = this.sectionContext[section] || {};
        
        // Create fully qualified title
        const originalTitle = chart.title || chart.opts?.title || '';
        enriched.fullTitle = this.createFullTitle(originalTitle, section, group);
        
        // Generate description if not present
        if (!enriched.description && !chart.opts?.description) {
            enriched.description = this.generateDescription(originalTitle, section, group);
        }
        
        // Add searchable text combining all relevant fields
        enriched.searchText = this.createSearchText(enriched, section, group);
        
        // Add metadata
        enriched.metadata = {
            ...enriched.metadata,
            section: section,
            sectionName: sectionInfo.prefix || section,
            group: group,
            originalTitle: originalTitle,
            keywords: this.extractKeywords(originalTitle, section),
            isSystemLevel: !originalTitle.toLowerCase().includes('cgroup'),
            isContainer: originalTitle.toLowerCase().includes('cgroup') || section === 'cgroups'
        };
        
        return enriched;
    }

    // Create a fully qualified title with context
    createFullTitle(title, section, group) {
        const sectionInfo = this.sectionContext[section] || {};
        const prefix = sectionInfo.prefix || section;
        
        // Don't duplicate if title already includes section context
        if (title.toLowerCase().includes(prefix.toLowerCase())) {
            return title;
        }
        
        // Handle special cases
        if (title.match(/^\d+\.\d+%?$/)) {
            // Just a number, needs full context
            return `${prefix} ${group} ${title}`;
        }
        
        if (title.includes('%') && !title.toLowerCase().includes('percent')) {
            // Percentage without context
            return `${prefix} ${title}`;
        }
        
        // Add section prefix
        return `${prefix} ${title}`;
    }

    // Generate a description based on title and context
    generateDescription(title, section, group) {
        const sectionInfo = this.sectionContext[section] || {};
        const lower = title.toLowerCase();
        
        // Check against known patterns
        for (const [pattern, description] of Object.entries(this.metricPatterns)) {
            if (lower.includes(pattern.toLowerCase())) {
                return `${sectionInfo.prefix || section} ${description}`;
            }
        }
        
        // Generate generic description
        if (section === 'cgroups') {
            return `Container/cgroup metric: ${title} in ${group}`;
        }
        
        return `${sectionInfo.description || 'System metric'}: ${title}`;
    }

    // Create comprehensive searchable text
    createSearchText(chart, section, group) {
        const parts = [
            chart.fullTitle,
            chart.originalTitle || chart.title,
            chart.description,
            section,
            group,
            this.sectionContext[section]?.prefix,
            ...(this.sectionContext[section]?.keywords || [])
        ];
        
        return parts.filter(Boolean).join(' ').toLowerCase();
    }

    // Extract relevant keywords from a title
    extractKeywords(title, section) {
        const keywords = [];
        const lower = title.toLowerCase();
        
        // Add section keywords
        if (this.sectionContext[section]) {
            keywords.push(...this.sectionContext[section].keywords);
        }
        
        // Extract metric type keywords
        if (lower.includes('busy') || lower.includes('usage')) keywords.push('utilization', 'usage');
        if (lower.includes('idle')) keywords.push('idle', 'available');
        if (lower.includes('rx') || lower.includes('receive')) keywords.push('incoming', 'receive', 'rx');
        if (lower.includes('tx') || lower.includes('transmit')) keywords.push('outgoing', 'transmit', 'tx');
        if (lower.includes('read')) keywords.push('read', 'input');
        if (lower.includes('write')) keywords.push('write', 'output');
        if (lower.includes('error')) keywords.push('error', 'failure', 'problem');
        if (lower.includes('latency')) keywords.push('latency', 'delay', 'response time');
        if (lower.includes('throughput')) keywords.push('throughput', 'bandwidth', 'speed');
        
        return [...new Set(keywords)]; // Remove duplicates
    }

    // Enrich all charts in a sections object
    enrichAllCharts(sections) {
        const enrichedCharts = [];
        
        for (const [sectionName, sectionData] of Object.entries(sections)) {
            if (!sectionData || !sectionData.groups) continue;
            
            for (const group of sectionData.groups) {
                if (!group.plots) continue;
                
                for (const plot of group.plots) {
                    if (!plot.opts?.title || !plot.data || plot.data.length < 2) continue;
                    
                    const enrichedChart = this.enrichChart(
                        {
                            title: plot.opts.title,
                            description: plot.opts.description,
                            data: plot.data,
                            opts: plot.opts,
                            plot: plot
                        },
                        sectionName,
                        group.name
                    );
                    
                    enrichedCharts.push(enrichedChart);
                }
            }
        }
        
        return enrichedCharts;
    }
}

// Export a singleton instance
export const chartEnricher = new ChartEnricher();