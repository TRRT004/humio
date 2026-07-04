#![allow(clippy::all, clippy::pedantic, clippy::nursery, unused)]

use std::thread;
use std::time::{Duration, Instant};
use std::io::Write;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

use humio::{HumanizedDevice, Mouse, PhysicalDevice, Point, TargetArea};

fn get_screen_center() -> Point {
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        Point::new(w / 2, h / 2)
    }
}

fn get_current_mouse() -> Point {
    let mut pt = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut pt);
    }
    Point::new(pt.x, pt.y)
}

fn filter_active_movement(samples: &[(u128, Point)]) -> Vec<(u128, Point)> {
    if samples.len() < 2 {
        return samples.to_vec();
    }
    // Find the first sample where the mouse moved significantly from the start (e.g. > 5 pixels)
    let start_pt = samples[0].1;
    let mut start_idx = 0;
    for (i, &(_, pt)) in samples.iter().enumerate() {
        let dist = (((pt.x - start_pt.x).pow(2) + (pt.y - start_pt.y).pow(2)) as f64).sqrt();
        if dist > 5.0 {
            start_idx = i.saturating_sub(5); // include a tiny bit of pre-movement
            break;
        }
    }

    // Find the last sample where the mouse was actively moving
    let mut end_idx = samples.len() - 1;
    for i in (start_idx..samples.len()).rev() {
        let current_pt = samples[i].1;
        let mut moved = false;
        // Check if there is any movement in the last 200ms
        for j in i.saturating_sub(20)..=i {
            let pt_j = samples[j].1;
            let dist = (((pt_j.x - current_pt.x).pow(2) + (pt_j.y - current_pt.y).pow(2)) as f64).sqrt();
            if dist > 2.0 {
                moved = true;
                break;
            }
        }
        if moved {
            end_idx = (i + 5).min(samples.len() - 1); // include a tiny bit of post-movement
            break;
        }
    }

    if start_idx >= end_idx {
        return samples.to_vec();
    }
    samples[start_idx..=end_idx].to_vec()
}

fn generate_velocity_chart_string(title: &str, samples: &[(u128, Point)]) -> String {
    let mut out = String::new();
    if samples.len() < 2 {
        out.push_str(&format!("{}: No movement recorded\n", title));
        return out;
    }
    let total_duration = samples.last().unwrap().0 - samples[0].0;
    out.push_str(&format!("\n--- Velocity Profile: {} ---\n", title));
    if total_duration == 0 {
        return out;
    }
    let intervals = 15;
    let step_duration = total_duration as f64 / intervals as f64;

    let mut velocities = Vec::new();
    let mut max_vel = 0.0;

    for i in 0..intervals {
        let start_time = samples[0].0 + (i as f64 * step_duration) as u128;
        let end_time = samples[0].0 + ((i + 1) as f64 * step_duration) as u128;

        // Find samples in this time window
        let pts: Vec<Point> = samples.iter()
            .filter(|(t, _)| *t >= start_time && *t <= end_time)
            .map(|(_, pt)| *pt)
            .collect();

        if pts.len() < 2 {
            velocities.push(0.0);
            continue;
        }

        // Calculate distance
        let mut dist = 0.0;
        for window in pts.windows(2) {
            let p1 = window[0];
            let p2 = window[1];
            dist += (((p2.x - p1.x).pow(2) + (p2.y - p1.y).pow(2)) as f64).sqrt();
        }
        let vel = dist / (end_time - start_time) as f64; // pixels/ms
        velocities.push(vel);
        if vel > max_vel {
            max_vel = vel;
        }
    }

    for (i, vel) in velocities.iter().enumerate() {
        let pct = i * 100 / intervals;
        let bar_len = if max_vel > 0.0 { (vel / max_vel * 30.0) as usize } else { 0 };
        let bar = "#".repeat(bar_len);
        out.push_str(&format!("{:3}% | {:<30} ({:.3} px/ms)\n", pct, bar, vel));
    }
    out
}

fn analyze_path(samples: &[(u128, Point)]) -> (f64, f64, f64, f64, f64) {
    if samples.len() < 2 {
        return (0.0, 0.0, 0.0, 0.0, 0.0);
    }
    let duration = (samples.last().unwrap().0 - samples[0].0) as f64; // ms

    let mut total_distance = 0.0;
    let mut max_speed = 0.0;

    for window in samples.windows(2) {
        let (t1, p1) = window[0];
        let (t2, p2) = window[1];
        let d = (((p2.x - p1.x).pow(2) + (p2.y - p1.y).pow(2)) as f64).sqrt();
        total_distance += d;
        let dt = (t2 - t1) as f64;
        if dt > 0.0 {
            let speed = d / dt;
            if speed > max_speed {
                max_speed = speed;
            }
        }
    }

    let avg_speed = if duration > 0.0 { total_distance / duration } else { 0.0 };

    let start_pt = samples[0].1;
    let end_pt = samples.last().unwrap().1;
    let direct_distance = (((end_pt.x - start_pt.x).pow(2) + (end_pt.y - start_pt.y).pow(2)) as f64).sqrt();
    let straightness = if total_distance > 0.0 { direct_distance / total_distance } else { 1.0 };

    (duration, total_distance, avg_speed, max_speed, straightness)
}

fn distance_from_line(pt: Point, start: Point, end: Point) -> f64 {
    let x1 = start.x as f64;
    let y1 = start.y as f64;
    let x2 = end.x as f64;
    let y2 = end.y as f64;
    let x0 = pt.x as f64;
    let y0 = pt.y as f64;

    let dy = y2 - y1;
    let dx = x2 - x1;
    let denom = (dy * dy + dx * dx).sqrt();
    if denom == 0.0 {
        return ((x0 - x1).powi(2) + (y0 - y1).powi(2)).sqrt();
    }
    (dy * x0 - dx * y0 + x2 * y1 - y2 * x1).abs() / denom
}

fn calculate_line_variance(samples: &[(u128, Point)]) -> (f64, f64, Vec<f64>) {
    if samples.len() < 2 {
        return (0.0, 0.0, Vec::new());
    }
    let start = samples[0].1;
    let end = samples.last().unwrap().1;

    let mut distances = Vec::new();
    let mut sum_squared = 0.0;
    let mut max_dev = 0.0;

    for &(_, pt) in samples {
        let d = distance_from_line(pt, start, end);
        distances.push(d);
        sum_squared += d * d;
        if d > max_dev {
            max_dev = d;
        }
    }

    let rms_deviation = (sum_squared / samples.len() as f64).sqrt();
    (rms_deviation, max_dev, distances)
}

fn main() {
    println!("=== Mouse Path Comparison Tool ===");
    println!("This tool will track and compare human vs script mouse trajectories.");
    println!("\nInstructions:");
    println!("1. Move your mouse to the top-left corner of the screen (0, 0).");
    println!("2. We will count down 3 seconds, then a 5-second recording starts.");
    println!("3. Move your mouse naturally from (0, 0) to the center of the screen.");
    println!("4. Do not worry about starting exactly on time or finishing early.");

    // Wait for mouse to get to (0, 0)
    loop {
        let pos = get_current_mouse();
        if pos.x < 20 && pos.y < 20 {
            break;
        }
        println!("Waiting for you to place mouse near (0, 0). Current position: {:?}", pos);
        thread::sleep(Duration::from_millis(500));
    }

    println!("\nMouse detected near (0, 0)!");
    for i in (1..=3).rev() {
        println!("Recording starts in {}...", i);
        thread::sleep(Duration::from_secs(1));
    }

    println!("\n>>> RECORDING STARTED! Move your mouse to the center now!");
    let record_start = Instant::now();
    let mut human_samples = Vec::new();

    while record_start.elapsed() < Duration::from_secs(5) {
        let t = record_start.elapsed().as_millis();
        human_samples.push((t, get_current_mouse()));
        thread::sleep(Duration::from_millis(10));
    }
    println!(">>> RECORDING FINISHED!");

    // Filter active movement
    let human_active = filter_active_movement(&human_samples);

    println!("\nReturning mouse to (0, 0) for script execution...");
    thread::sleep(Duration::from_secs(2));

    // Force mouse back to (0, 0)
    let physical = PhysicalDevice::new().expect("Failed to init physical device");
    let mut dev = HumanizedDevice::new(physical);
    let _ = dev.move_mouse(Point::new(0, 0));
    thread::sleep(Duration::from_secs(1));

    // Record script movement
    println!("\n>>> SCRIPT EXECUTING MOVEMENT...");

    // Spawn tracking thread to poll cursor pos
    let tracking_active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let t_active = tracking_active.clone();

    let tracker_handle = thread::spawn(move || {
        let tracker_start = Instant::now();
        let mut script_samples = Vec::new();
        while t_active.load(std::sync::atomic::Ordering::Relaxed) {
            let t = tracker_start.elapsed().as_millis();
            script_samples.push((t, get_current_mouse()));
            thread::sleep(Duration::from_millis(10));
        }
        script_samples
    });

    // Execute the script's mouse movement to the screen center
    let center = get_screen_center();
    let target = TargetArea::Point(center);
    dev.click_area(&target, enigo::Button::Left, false).expect("Script movement failed");

    // Wait and stop tracking thread
    thread::sleep(Duration::from_millis(500));
    tracking_active.store(false, std::sync::atomic::Ordering::Relaxed);
    let script_samples = tracker_handle.join().unwrap();
    let script_active = filter_active_movement(&script_samples);

    // Analyze paths
    let (h_dur, h_dist, h_avg, h_max, h_str) = analyze_path(&human_active);
    let (s_dur, s_dist, s_avg, s_max, s_str) = analyze_path(&script_active);

    // Calculate line deviation
    let (h_rms, h_max_dev, h_step_devs) = calculate_line_variance(&human_active);
    let (s_rms, s_max_dev, s_step_devs) = calculate_line_variance(&script_active);

    let h_overshoot = if human_active.len() >= 2 {
        let start = human_active[0].1;
        let target_dist = (((center.x - start.x).pow(2) + (center.y - start.y).pow(2)) as f64).sqrt();
        let max_dist = human_active.iter().map(|(_, pt)| (((pt.x - start.x).pow(2) + (pt.y - start.y).pow(2)) as f64).sqrt()).fold(0.0, f64::max);
        (max_dist - target_dist).max(0.0)
    } else { 0.0 };

    let s_overshoot = if script_active.len() >= 2 {
        let start = script_active[0].1;
        let target_dist = (((center.x - start.x).pow(2) + (center.y - start.y).pow(2)) as f64).sqrt();
        let max_dist = script_active.iter().map(|(_, pt)| (((pt.x - start.x).pow(2) + (pt.y - start.y).pow(2)) as f64).sqrt()).fold(0.0, f64::max);
        (max_dist - target_dist).max(0.0)
    } else { 0.0 };

    let h_landing_err = if human_active.len() >= 1 {
        let end = human_active.last().unwrap().1;
        (((center.x - end.x).pow(2) + (center.y - end.y).pow(2)) as f64).sqrt()
    } else { 0.0 };

    let s_landing_err = if script_active.len() >= 1 {
        let end = script_active.last().unwrap().1;
        (((center.x - end.x).pow(2) + (center.y - end.y).pow(2)) as f64).sqrt()
    } else { 0.0 };

    // Format output log
    let mut log_content = String::new();
    log_content.push_str("==============================================\n");
    log_content.push_str("                 COMPARISON                   \n");
    log_content.push_str("==============================================\n");
    log_content.push_str("Metric                 | Human       | Script      \n");
    log_content.push_str("-----------------------+-------------+-------------\n");
    log_content.push_str(&format!("Duration (ms)          | {:11.1} | {:11.1}\n", h_dur, s_dur));
    log_content.push_str(&format!("Total Distance (px)    | {:11.1} | {:11.1}\n", h_dist, s_dist));
    log_content.push_str(&format!("Avg Speed (px/ms)      | {:11.3} | {:11.3}\n", h_avg, s_avg));
    log_content.push_str(&format!("Max Speed (px/ms)      | {:11.3} | {:11.3}\n", h_max, s_max));
    log_content.push_str(&format!("Straightness (0 to 1)  | {:11.3} | {:11.3}\n", h_str, s_str));
    log_content.push_str(&format!("Overshoot (px)         | {:11.1} | {:11.1}\n", h_overshoot, s_overshoot));
    log_content.push_str(&format!("Landing Error (px)     | {:11.1} | {:11.1}\n", h_landing_err, s_landing_err));
    log_content.push_str(&format!("RMS Dev from Line (px) | {:11.3} | {:11.3}\n", h_rms, s_rms));
    log_content.push_str(&format!("Max Dev from Line (px) | {:11.3} | {:11.3}\n", h_max_dev, s_max_dev));

    log_content.push_str(&generate_velocity_chart_string("HUMAN", &human_active));
    log_content.push_str(&generate_velocity_chart_string("SCRIPT", &script_active));

    log_content.push_str("\n==============================================\n");
    log_content.push_str("        STEP-BY-STEP DEV FROM LINE            \n");
    log_content.push_str("==============================================\n");

    log_content.push_str("\n--- HUMAN PATH DETAILS ---\n");
    for (i, &(t, pt)) in human_active.iter().enumerate() {
        let dev = h_step_devs[i];
        log_content.push_str(&format!(
            "[Step {:3}] t={:4}ms pos=({:4}, {:4}) dev_from_line={:6.2}px\n",
            i + 1, t, pt.x, pt.y, dev
        ));
    }

    log_content.push_str("\n--- SCRIPT PATH DETAILS ---\n");
    for (i, &(t, pt)) in script_active.iter().enumerate() {
        let dev = s_step_devs[i];
        log_content.push_str(&format!(
            "[Step {:3}] t={:4}ms pos=({:4}, {:4}) dev_from_line={:6.2}px\n",
            i + 1, t, pt.x, pt.y, dev
        ));
    }

    // Write to file
    let file_path = "compare_results.log";
    if let Ok(mut file) = std::fs::File::create(file_path) {
        let _ = file.write_all(log_content.as_bytes());
        println!("\nComparison complete! Detailed results logged to: {}", file_path);
    } else {
        println!("\nError: Failed to write results to {}", file_path);
    }

    unsafe {
        timeEndPeriod(1);
    }
}
