# Poly Language Profiling Guide

## Overview

This guide covers profiling and performance analysis techniques for Poly programs.

**Note:** Some functions used in this guide (like `time_now()`, `get_memory_usage()`, `num_cpus()`) are standard library functions. See the Standard Library section for details.

---

## 1. Basic Profiling

### Measure Execution Time

~~~poly fragment
// Measure execution time
fn benchmark(name: ustring, iterations: i32, fn: () -> T): BenchmarkResult
    var times Vec<i64> := []
    
    loop: iteration in iterations
        var start := time_now()
        fn()
        var duration := time_now() - start
        times.push(duration)
    end loop
    
    var avg := times.iter().sum() / iterations
    var min := times.iter().min()
    var max := times.iter().max()
    
    return BenchmarkResult {
        name: name,
        iterations: iterations,
        avg_ms: avg,
        min_ms: min,
        max_ms: max
    }
end fn

// Usage
var result := benchmark("sort_array", 1000, || sort_array(data))
put "Average: " + result.avg_ms.to_string() + "ms"
put "Min: " + result.min_ms.to_string() + "ms"
put "Max: " + result.max_ms.to_string() + "ms"
~~~

### Detect Memory Leaks

~~~poly
// Detect memory leaks
fn detect_leaks(iterations: i32)
    var initial_memory := get_memory_usage()
    
    loop: iteration in iterations
        // Code that might leak memory
        var data := allocate_large_array()
        // ... process data
        // If not properly freed, memory increases
    end loop
    
    var final_memory := get_memory_usage()
    var leaked := final_memory - initial_memory
    
    if leaked > 0,
        warn "Potential memory leak: " + leaked.to_string() + " bytes"
    else
        info "No memory leaks detected"
    end if
end fn
~~~

---

## 2. Advanced Profiling

### Profile Function Calls

~~~poly fragment
// Profile function calls
fn profile_calls(fn: () -> T, iterations: i32): ProfileResult
    var call_count i32 := 0
    var total_time i64 := 0
    
    var profiled_fn := || {
        add call_count, 1
        var start := time_now()
        var result := fn()
        var duration := time_now() - start
        add total_time, duration
        return result
    }
    
    loop: iteration in iterations
        profiled_fn()
    end loop
    
    return ProfileResult {
        call_count: call_count,
        total_time_ms: total_time,
        avg_time_ms: total_time / call_count
    }
end fn

// Usage
var result := profile_calls(|| process_data(), 1000)
put "Calls: " + result.call_count.to_string()
put "Total time: " + result.total_time_ms.to_string() + "ms"
put "Average: " + result.avg_time_ms.to_string() + "ms"
~~~

### Detect Hotspots

~~~poly
// Detect hotspots
fn detect_hotspots(functions: Vec<(ustring, fn() -> T)>): Vec<Hotspot>
    var hotspots Vec<Hotspot> := []
    
    loop: function in functions
        var result := benchmark(name, 100, func)
        hotspots.push(Hotspot {
            name: name,
            avg_ms: result.avg_ms,
            percentage: 0.0  // Calculated later
        })
    end loop
    
    // Calculate percentages
    var total_time := hotspots.iter().map(|h| h.avg_ms).sum()
    loop: hotspot in hotspots.iter_mut()
        set hotspot.percentage to (hotspot.avg_ms as f64) / (total_time as f64) * 100.0
    end loop
    
    // Sort by time (descending)
    hotspots.sort_by(|a, b| b.avg_ms.compare(a.avg_ms))
    
    return hotspots
end fn
~~~

---

## 3. I/O Profiling

### Profile File Operations

~~~poly
// Profile file operations
fn profile_file_io(filename: ustring, iterations: i32): FileIOProfile
    // Profile writes
    var write_times Vec<i64> := []
    loop: iteration in iterations
        var start := time_now()
        put "test data" > filename
        var duration := time_now() - start
        write_times.push(duration)
    end loop
    
    // Profile reads
    var read_times Vec<i64> := []
    loop: iteration in iterations
        var start := time_now()
        var content ustring := get < filename
        var duration := time_now() - start
        read_times.push(duration)
    end loop
    
    delete_file(filename)
    
    return FileIOProfile {
        write_avg_ms: write_times.iter().sum() / iterations,
        read_avg_ms: read_times.iter().sum() / iterations
    }
end fn
~~~

### Profile Network Operations

~~~poly
// Profile network operations
fn profile_network(url: ustring, iterations: i32): NetworkProfile
    var times Vec<i64> := []
    var errors i32 := 0
    
    loop: iteration in iterations
        var start := time_now()
        match get --timeout 5000 < url
            Ok(_),
                var duration := time_now() - start
                times.push(duration)
            Error(_), errors = errors + 1
        end match
    end loop
    
    return NetworkProfile {
        avg_ms: times.iter().sum() / times.len(),
        min_ms: times.iter().min(),
        max_ms: times.iter().max(),
        error_rate: (errors as f64) / (iterations as f64) * 100.0
    }
end fn
~~~

---

## 4. Concurrency Profiling

### Profile Thread Usage

~~~poly fragment
// Profile thread usage
fn profile_threads(iterations: i32): ThreadProfile
    var thread_count i32 := 0
    var creation_times Vec<i64> := []
    
    loop: iteration in iterations
        var start := time_now()
        spawn(|| {
            // Thread work
            add thread_count, 1
        })
        var duration := time_now() - start
        creation_times.push(duration)
    end loop
    
    return ThreadProfile {
        avg_creation_ms: creation_times.iter().sum() / iterations,
        threads_created: thread_count
    }
end fn
~~~

### Profile Lock Contention

~~~poly
// Profile lock contention
fn profile_locks(iterations: i32): LockProfile
    var lock := Mutex::new(0)
    var contention_count i32 := 0
    var wait_times Vec<i64> := []
    
    loop: iteration in iterations
        var start := time_now()
        lock.lock()
        var duration := time_now() - start
        wait_times.push(duration)
        
        if duration > 10, // Contention threshold
            add contention_count, 1
        end if
        
        // Critical section
        sleep(1)
        lock.unlock()
    end loop
    
    return LockProfile {
        avg_wait_ms: wait_times.iter().sum() / iterations,
        contention_count: contention_count,
        contention_rate: (contention_count as f64) / (iterations as f64) * 100.0
    }
end fn
~~~

---

## 5. Reporting

### Generate Profiling Report

~~~poly
// Generate profiling report
fn generate_report(results: Vec<ProfileResult>): ustring
    var report ustring := "# Performance Report\n\n"
    
    // Summary
    add report, "## Summary\n\n"
    add report, "- Total tests: " + results.len().to_string() + "\n"
    var total_time := results.iter().map(|r| r.time_ms).sum()
    add report, "- Total time: " + total_time.to_string() + "ms\n\n"
    
    // Detailed results
    add report, "## Detailed Results\n\n"
    add report, "| Test | Time (ms) | Status |\n"
    add report, "|------|-----------|--------|\n"
    
    loop: result in results
        var status := if result.passed,"PASS" else "FAIL" end if
        add report, "| " + result.name + " | " + result.time_ms.to_string() + " | " + status + " |\n"
    end loop
    
    return report
end fn

// Save report
fn save_report(report: ustring, filename: ustring)
    put report > filename
    put "Report saved to: " + filename
end fn
~~~

### Analyze Profiling Results

~~~poly
// Analyze profiling results
fn analyze_results(results: Vec<ProfileResult>): Vec<Recommendation>
    var recommendations Vec<Recommendation> := []
    
    loop: result in results
        // Check for slow operations
        if result.time_ms > 1000,
            recommendations.push(Recommendation {
                issue: "Slow operation: " + result.name,
                suggestion: "Consider optimizing or caching",
                priority: "high"
            })
        end if
        
        // Check for memory issues
        if result.memory_allocated > 1000000,
            recommendations.push(Recommendation {
                issue: "High memory usage: " + result.name,
                suggestion: "Consider streaming or chunking",
                priority: "high"
            })
        end if
        
        // Check for error rates
        if result.error_rate > 0.01,
            recommendations.push(Recommendation {
                issue: "High error rate: " + result.name,
                suggestion: "Add error handling or retry logic",
                priority: "medium"
            })
        end if
    end loop
    
    return recommendations
end fn
~~~

---

## Summary

1. **Basic Profiling**: Measure execution time, detect memory leaks
2. **Advanced Profiling**: Profile function calls, detect hotspots
3. **I/O Profiling**: Profile file and network operations
4. **Concurrency Profiling**: Profile thread usage and lock contention
5. **Reporting**: Generate reports and analyze results
