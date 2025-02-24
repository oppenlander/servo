/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Configuration options for a single run of the servo application. Created
//! from command line arguments.

use geometry::ScreenPx;

use euclid::size::{Size2D, TypedSize2D};
use getopts::Options;
use num_cpus;
use std::collections::HashSet;
use std::cmp;
use std::env;
use std::io::{self, Read, Write};
use std::fs::{File, PathExt};
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering, ATOMIC_BOOL_INIT};
use url::{self, Url};

/// Global flags for Servo, currently set on the command line.
#[derive(Clone)]
pub struct Opts {
    /// The initial URL to load.
    pub url: Option<Url>,

    /// How many threads to use for CPU painting (`-t`).
    ///
    /// Note that painting is sequentialized when using GPU painting.
    pub paint_threads: usize,

    /// True to use GPU painting via Skia-GL, false to use CPU painting via Skia (`-g`). Note that
    /// compositing is always done on the GPU.
    pub gpu_painting: bool,

    /// The maximum size of each tile in pixels (`-s`).
    pub tile_size: usize,

    /// The ratio of device pixels per px at the default scale. If unspecified, will use the
    /// platform default setting.
    pub device_pixels_per_px: Option<f32>,

    /// `None` to disable the time profiler or `Some` with an interval in seconds to enable it and
    /// cause it to produce output on that interval (`-p`).
    pub time_profiler_period: Option<f64>,

    /// `None` to disable the memory profiler or `Some` with an interval in seconds to enable it
    /// and cause it to produce output on that interval (`-m`).
    pub mem_profiler_period: Option<f64>,

    /// Enable experimental web features (`-e`).
    pub enable_experimental: bool,

    /// The number of threads to use for layout (`-y`). Defaults to 1, which results in a recursive
    /// sequential algorithm.
    pub layout_threads: usize,

    pub nonincremental_layout: bool,

    pub nossl: bool,

    /// Where to load userscripts from, if any. An empty string will load from
    /// the resources/user-agent-js directory, and if the option isn't passed userscripts
    /// won't be loaded
    pub userscripts: Option<String>,

    pub user_stylesheets: Vec<(Vec<u8>, Url)>,

    pub output_file: Option<String>,

    /// Replace unpaires surrogates in DOM strings with U+FFFD.
    /// See https://github.com/servo/servo/issues/6564
    pub replace_surrogates: bool,

    /// Log GC passes and their durations.
    pub gc_profile: bool,

    pub headless: bool,
    pub hard_fail: bool,

    /// True if we should bubble intrinsic widths sequentially (`-b`). If this is true, then
    /// intrinsic widths are computed as a separate pass instead of during flow construction. You
    /// may wish to turn this flag on in order to benchmark style recalculation against other
    /// browser engines.
    pub bubble_inline_sizes_separately: bool,

    /// True if we should show borders on all layers and tiles for
    /// debugging purposes (`--show-debug-borders`).
    pub show_debug_borders: bool,

    /// True if we should show borders on all fragments for debugging purposes
    /// (`--show-debug-fragment-borders`).
    pub show_debug_fragment_borders: bool,

    /// True if we should paint tiles with overlays based on which thread painted them.
    pub show_debug_parallel_paint: bool,

    /// True if we should paint borders around flows based on which thread painted them.
    pub show_debug_parallel_layout: bool,

    /// True if we should paint tiles a random color whenever they're repainted. Useful for
    /// debugging invalidation.
    pub paint_flashing: bool,

    /// If set with --disable-text-aa, disable antialiasing on fonts. This is primarily useful for reftests
    /// where pixel perfect results are required when using fonts such as the Ahem
    /// font for layout tests.
    pub enable_text_antialiasing: bool,

    /// If set with --disable-canvas-aa, disable antialiasing on the HTML canvas element.
    /// Like --disable-text-aa, this is useful for reftests where pixel perfect results are required.
    pub enable_canvas_antialiasing: bool,

    /// True if each step of layout is traced to an external JSON file
    /// for debugging purposes. Settings this implies sequential layout
    /// and paint.
    pub trace_layout: bool,

    /// If true, instrument the runtime for each task created and dump
    /// that information to a JSON file that can be viewed in the task
    /// profile viewer.
    pub profile_tasks: bool,

    /// `None` to disable devtools or `Some` with a port number to start a server to listen to
    /// remote Firefox devtools connections.
    pub devtools_port: Option<u16>,

    /// `None` to disable WebDriver or `Some` with a port number to start a server to listen to
    /// remote WebDriver commands.
    pub webdriver_port: Option<u16>,

    /// The initial requested size of the window.
    pub initial_window_size: TypedSize2D<ScreenPx, u32>,

    /// An optional string allowing the user agent to be set for testing.
    pub user_agent: Option<String>,

    /// Whether to run in multiprocess mode.
    pub multiprocess: bool,

    /// Dumps the flow tree after a layout.
    pub dump_flow_tree: bool,

    /// Dumps the display list after a layout.
    pub dump_display_list: bool,

    /// Dumps the display list in JSON form after a layout.
    pub dump_display_list_json: bool,

    /// Dumps the display list after optimization (post layout, at painting time).
    pub dump_display_list_optimized: bool,

    /// Emits notifications when there is a relayout.
    pub relayout_event: bool,

    /// Whether to show an error when display list geometry escapes flow overflow regions.
    pub validate_display_list_geometry: bool,

    /// A specific path to find required resources (such as user-agent.css).
    pub resources_path: Option<String>,

    /// Whether MIME sniffing should be used
    pub sniff_mime_types: bool,

    /// Whether Style Sharing Cache is used
    pub disable_share_style_cache: bool,

    /// Whether to run absolute position calculation and display list construction in parallel.
    pub parallel_display_list_building: bool,

    /// True to exit after the page load (`-x`).
    pub exit_after_load: bool,
}

fn print_usage(app: &str, opts: &Options) {
    let message = format!("Usage: {} [ options ... ] [URL]\n\twhere options include", app);
    println!("{}", opts.usage(&message));
}

pub fn print_debug_usage(app: &str) -> ! {
    fn print_option(name: &str, description: &str) {
        println!("\t{:<35} {}", name, description);
    }

    println!("Usage: {} debug option,[options,...]\n\twhere options include\n\nOptions:", app);

    print_option("bubble-widths", "Bubble intrinsic widths separately like other engines.");
    print_option("disable-text-aa", "Disable antialiasing of rendered text.");
    print_option("dump-flow-tree", "Print the flow tree after each layout.");
    print_option("dump-display-list", "Print the display list after each layout.");
    print_option("dump-display-list-json", "Print the display list in JSON form.");
    print_option("dump-display-list-optimized", "Print optimized display list (at paint time).");
    print_option("relayout-event", "Print notifications when there is a relayout.");
    print_option("profile-tasks", "Instrument each task, writing the output to a file.");
    print_option("show-compositor-borders", "Paint borders along layer and tile boundaries.");
    print_option("show-fragment-borders", "Paint borders along fragment boundaries.");
    print_option("show-parallel-paint", "Overlay tiles with colors showing which thread painted them.");
    print_option("show-parallel-layout", "Mark which thread laid each flow out with colors.");
    print_option("paint-flashing", "Overlay repainted areas with a random color.");
    print_option("trace-layout", "Write layout trace to an external file for debugging.");
    print_option("validate-display-list-geometry",
                 "Display an error when display list geometry escapes overflow region.");
    print_option("disable-share-style-cache",
                 "Disable the style sharing cache.");
    print_option("parallel-display-list-building", "Build display lists in parallel.");
    print_option("replace-surrogates", "Replace unpaires surrogates in DOM strings with U+FFFD. \
                                        See https://github.com/servo/servo/issues/6564");
    print_option("gc-profile", "Log GC passes and their durations.");

    println!("");

    process::exit(0)
}

fn args_fail(msg: &str) -> ! {
    let mut stderr = io::stderr();
    stderr.write_all(msg.as_bytes()).unwrap();
    stderr.write_all(b"\n").unwrap();
    process::exit(1)
}

// Always use CPU painting on android.

#[cfg(target_os="android")]
static FORCE_CPU_PAINTING: bool = true;

#[cfg(not(target_os="android"))]
static FORCE_CPU_PAINTING: bool = false;

pub fn default_opts() -> Opts {
    Opts {
        url: Some(Url::parse("about:blank").unwrap()),
        paint_threads: 1,
        gpu_painting: false,
        tile_size: 512,
        device_pixels_per_px: None,
        time_profiler_period: None,
        mem_profiler_period: None,
        enable_experimental: false,
        layout_threads: 1,
        nonincremental_layout: false,
        nossl: false,
        userscripts: None,
        user_stylesheets: Vec::new(),
        output_file: None,
        replace_surrogates: false,
        gc_profile: false,
        headless: true,
        hard_fail: true,
        bubble_inline_sizes_separately: false,
        show_debug_borders: false,
        show_debug_fragment_borders: false,
        show_debug_parallel_paint: false,
        show_debug_parallel_layout: false,
        paint_flashing: false,
        enable_text_antialiasing: false,
        enable_canvas_antialiasing: false,
        trace_layout: false,
        devtools_port: None,
        webdriver_port: None,
        initial_window_size: Size2D::typed(800, 600),
        user_agent: None,
        multiprocess: false,
        dump_flow_tree: false,
        dump_display_list: false,
        dump_display_list_json: false,
        dump_display_list_optimized: false,
        relayout_event: false,
        validate_display_list_geometry: false,
        profile_tasks: false,
        resources_path: None,
        sniff_mime_types: false,
        disable_share_style_cache: false,
        parallel_display_list_building: false,
        exit_after_load: false,
    }
}

pub fn from_cmdline_args(args: &[String]) {
    let (app_name, args) = args.split_first().unwrap();

    let mut opts = Options::new();
    opts.optflag("c", "cpu", "CPU painting (default)");
    opts.optflag("g", "gpu", "GPU painting");
    opts.optopt("o", "output", "Output file", "output.png");
    opts.optopt("s", "size", "Size of tiles", "512");
    opts.optopt("", "device-pixel-ratio", "Device pixels per px", "");
    opts.optflag("e", "experimental", "Enable experimental web features");
    opts.optopt("t", "threads", "Number of paint threads", "1");
    opts.optflagopt("p", "profile", "Profiler flag and output interval", "10");
    opts.optflagopt("m", "memory-profile", "Memory profiler flag and output interval", "10");
    opts.optflag("x", "exit", "Exit after load flag");
    opts.optopt("y", "layout-threads", "Number of threads to use for layout", "1");
    opts.optflag("i", "nonincremental-layout", "Enable to turn off incremental layout.");
    opts.optflag("", "no-ssl", "Disables ssl certificate verification.");
    opts.optflagopt("", "userscripts",
                    "Uses userscripts in resources/user-agent-js, or a specified full path","");
    opts.optmulti("", "user-stylesheet",
                  "A user stylesheet to be added to every document", "file.css");
    opts.optflag("z", "headless", "Headless mode");
    opts.optflag("f", "hard-fail", "Exit on task failure instead of displaying about:failure");
    opts.optflagopt("", "devtools", "Start remote devtools server on port", "6000");
    opts.optflagopt("", "webdriver", "Start remote WebDriver server on port", "7000");
    opts.optopt("", "resolution", "Set window resolution.", "800x600");
    opts.optopt("u", "user-agent", "Set custom user agent string", "NCSA Mosaic/1.0 (X11;SunOS 4.1.4 sun4m)");
    opts.optflag("M", "multiprocess", "Run in multiprocess mode");
    opts.optopt("Z", "debug",
                "A comma-separated string of debug options. Pass help to show available options.", "");
    opts.optflag("h", "help", "Print this message");
    opts.optopt("", "resources-path", "Path to find static resources", "/home/servo/resources");
    opts.optflag("", "sniff-mime-types" , "Enable MIME sniffing");

    let opt_match = match opts.parse(args) {
        Ok(m) => m,
        Err(f) => args_fail(&f.to_string()),
    };

    if opt_match.opt_present("h") || opt_match.opt_present("help") {
        print_usage(app_name, &opts);
        process::exit(0);
    };

    let debug_string = match opt_match.opt_str("Z") {
        Some(string) => string,
        None => String::new()
    };
    let mut debug_options = HashSet::new();
    for split in debug_string.split(',') {
        debug_options.insert(split.clone());
    }
    if debug_options.contains(&"help") {
        print_debug_usage(app_name)
    }

    let cwd = env::current_dir().unwrap();
    let url = if opt_match.free.is_empty() {
        print_usage(app_name, &opts);
        args_fail("servo asks that you provide a URL")
    } else {
        let ref url = opt_match.free[0];
        match Url::parse(url) {
            Ok(url) => url,
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                if Path::new(url).exists() {
                    Url::from_file_path(&*cwd.join(url)).unwrap()
                } else {
                    args_fail(&format!("File not found: {}", url))
                }
            }
            Err(_) => panic!("URL parsing failed"),
        }
    };

    let tile_size: usize = match opt_match.opt_str("s") {
        Some(tile_size_str) => tile_size_str.parse().unwrap(),
        None => 512,
    };

    let device_pixels_per_px = opt_match.opt_str("device-pixel-ratio").map(|dppx_str|
        dppx_str.parse().unwrap()
    );

    let mut paint_threads: usize = match opt_match.opt_str("t") {
        Some(paint_threads_str) => paint_threads_str.parse().unwrap(),
        None => cmp::max(num_cpus::get() * 3 / 4, 1),
    };

    // If only the flag is present, default to a 5 second period for both profilers.
    let time_profiler_period = opt_match.opt_default("p", "5").map(|period| {
        period.parse().unwrap()
    });
    let mem_profiler_period = opt_match.opt_default("m", "5").map(|period| {
        period.parse().unwrap()
    });

    let gpu_painting = !FORCE_CPU_PAINTING && opt_match.opt_present("g");

    let mut layout_threads: usize = match opt_match.opt_str("y") {
        Some(layout_threads_str) => layout_threads_str.parse().unwrap(),
        None => cmp::max(num_cpus::get() * 3 / 4, 1),
    };

    let nonincremental_layout = opt_match.opt_present("i");
    let nossl = opt_match.opt_present("no-ssl");

    let mut bubble_inline_sizes_separately = debug_options.contains(&"bubble-widths");
    let trace_layout = debug_options.contains(&"trace-layout");
    if trace_layout {
        paint_threads = 1;
        layout_threads = 1;
        bubble_inline_sizes_separately = true;
    }

    let devtools_port = opt_match.opt_default("devtools", "6000").map(|port| {
        port.parse().unwrap()
    });

    let webdriver_port = opt_match.opt_default("webdriver", "7000").map(|port| {
        port.parse().unwrap()
    });

    let initial_window_size = match opt_match.opt_str("resolution") {
        Some(res_string) => {
            let res: Vec<u32> = res_string.split('x').map(|r| r.parse().unwrap()).collect();
            Size2D::typed(res[0], res[1])
        }
        None => {
            Size2D::typed(800, 600)
        }
    };

    let user_stylesheets = opt_match.opt_strs("user-stylesheet").iter().map(|filename| {
        let path = cwd.join(filename);
        let url = Url::from_file_path(&path).unwrap();
        let mut contents = Vec::new();
        File::open(path)
            .unwrap_or_else(|err| args_fail(&format!("Couldn’t open {}: {}", filename, err)))
            .read_to_end(&mut contents)
            .unwrap_or_else(|err| args_fail(&format!("Couldn’t read {}: {}", filename, err)));
        (contents, url)
    }).collect();

    let opts = Opts {
        url: Some(url),
        paint_threads: paint_threads,
        gpu_painting: gpu_painting,
        tile_size: tile_size,
        device_pixels_per_px: device_pixels_per_px,
        time_profiler_period: time_profiler_period,
        mem_profiler_period: mem_profiler_period,
        enable_experimental: opt_match.opt_present("e"),
        layout_threads: layout_threads,
        nonincremental_layout: nonincremental_layout,
        nossl: nossl,
        userscripts: opt_match.opt_default("userscripts", ""),
        user_stylesheets: user_stylesheets,
        output_file: opt_match.opt_str("o"),
        replace_surrogates: debug_options.contains(&"replace-surrogates"),
        gc_profile: debug_options.contains(&"gc-profile"),
        headless: opt_match.opt_present("z"),
        hard_fail: opt_match.opt_present("f"),
        bubble_inline_sizes_separately: bubble_inline_sizes_separately,
        profile_tasks: debug_options.contains(&"profile-tasks"),
        trace_layout: trace_layout,
        devtools_port: devtools_port,
        webdriver_port: webdriver_port,
        initial_window_size: initial_window_size,
        user_agent: opt_match.opt_str("u"),
        multiprocess: opt_match.opt_present("M"),
        show_debug_borders: debug_options.contains(&"show-compositor-borders"),
        show_debug_fragment_borders: debug_options.contains(&"show-fragment-borders"),
        show_debug_parallel_paint: debug_options.contains(&"show-parallel-paint"),
        show_debug_parallel_layout: debug_options.contains(&"show-parallel-layout"),
        paint_flashing: debug_options.contains(&"paint-flashing"),
        enable_text_antialiasing: !debug_options.contains(&"disable-text-aa"),
        enable_canvas_antialiasing: !debug_options.contains(&"disable-canvas-aa"),
        dump_flow_tree: debug_options.contains(&"dump-flow-tree"),
        dump_display_list: debug_options.contains(&"dump-display-list"),
        dump_display_list_json: debug_options.contains(&"dump-display-list-json"),
        dump_display_list_optimized: debug_options.contains(&"dump-display-list-optimized"),
        relayout_event: debug_options.contains(&"relayout-event"),
        validate_display_list_geometry: debug_options.contains(&"validate-display-list-geometry"),
        resources_path: opt_match.opt_str("resources-path"),
        sniff_mime_types: opt_match.opt_present("sniff-mime-types"),
        disable_share_style_cache: debug_options.contains(&"disable-share-style-cache"),
        parallel_display_list_building: debug_options.contains(&"parallel-display-list-building"),
        exit_after_load: opt_match.opt_present("x"),
    };

    set_defaults(opts);
}

static EXPERIMENTAL_ENABLED: AtomicBool = ATOMIC_BOOL_INIT;

/// Turn on experimental features globally. Normally this is done
/// during initialization by `set` or `from_cmdline_args`, but
/// tests that require experimental features will also set it.
pub fn set_experimental_enabled(new_value: bool) {
    EXPERIMENTAL_ENABLED.store(new_value, Ordering::SeqCst);
}

pub fn experimental_enabled() -> bool {
    EXPERIMENTAL_ENABLED.load(Ordering::SeqCst)
}

// Make Opts available globally. This saves having to clone and pass
// opts everywhere it is used, which gets particularly cumbersome
// when passing through the DOM structures.
static mut DEFAULT_OPTIONS: *mut Opts = 0 as *mut Opts;
const INVALID_OPTIONS: *mut Opts = 0x01 as *mut Opts;

lazy_static! {
    static ref OPTIONS: Opts = {
        let opts = unsafe {
            let initial = if !DEFAULT_OPTIONS.is_null() {
                let opts = Box::from_raw(DEFAULT_OPTIONS);
                *opts
            } else {
                default_opts()
            };
            DEFAULT_OPTIONS = INVALID_OPTIONS;
            initial
        };
        set_experimental_enabled(opts.enable_experimental);
        opts
    };
}

pub fn set_defaults(opts: Opts) {
    unsafe {
        assert!(DEFAULT_OPTIONS.is_null());
        assert!(DEFAULT_OPTIONS != INVALID_OPTIONS);
        let box_opts = box opts;
        DEFAULT_OPTIONS = Box::into_raw(box_opts);
    }
}

#[inline]
pub fn get() -> &'static Opts {
    &OPTIONS
}
