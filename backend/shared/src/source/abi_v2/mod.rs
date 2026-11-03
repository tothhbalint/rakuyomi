pub mod html;
pub mod html_dom;
pub mod mem;
pub mod model;
pub mod net;
pub mod settings;
pub mod source;
pub mod std;
pub mod store;

pub mod defaults;
pub mod env;

pub use source::AbiV2Source;

use anyhow::Result;
use wasmi::{Caller, Linker};

use store::WasmStore;

pub fn register_imports(linker: &mut Linker<WasmStore>) -> Result<()> {
    // std
    linker.func_wrap("std", "abort", std::abort)?;
    linker.func_wrap("std", "print", std::print)?;
    linker.func_wrap("std", "destroy", std::destroy)?;
    linker.func_wrap("std", "buffer_len", std::buffer_len)?;
    linker.func_wrap("std", "read_buffer", std::read_buffer)?;
    linker.func_wrap("std", "current_date", std::current_date)?;
    linker.func_wrap("std", "utc_offset", std::utc_offset)?;
    linker.func_wrap("std", "parse_date", std::parse_date)?;

    // env
    linker.func_wrap("env", "abort", env::abort)?;
    linker.func_wrap("env", "print", env::print)?;
    linker.func_wrap("env", "sleep", env::sleep)?;
    linker.func_wrap("env", "send_partial_result", env::send_partial_result)?;

    // defaults
    linker.func_wrap("defaults", "get", defaults::get)?;
    linker.func_wrap("defaults", "set", defaults::set)?;

    // net
    linker.func_wrap("net", "init", net::init)?;
    linker.func_wrap("net", "send", net::send)?;
    linker.func_wrap("net", "send_all", net::send_all)?;
    linker.func_wrap("net", "set_url", net::set_url)?;
    linker.func_wrap("net", "set_header", net::set_header)?;
    linker.func_wrap("net", "set_body", net::set_body)?;
    linker.func_wrap("net", "set_timeout", net::set_timeout)?;
    linker.func_wrap("net", "data_len", net::data_len)?;
    linker.func_wrap("net", "read_data", net::read_data)?;
    linker.func_wrap("net", "get_image", net::get_image)?;
    linker.func_wrap("net", "get_status_code", net::get_status_code)?;
    linker.func_wrap("net", "get_url", net::get_url)?;
    linker.func_wrap("net", "get_header", net::get_header)?;
    linker.func_wrap("net", "html", net::html)?;
    linker.func_wrap("net", "set_rate_limit", net::set_rate_limit)?;

    // html
    linker.func_wrap("html", "parse", html::parse)?;
    linker.func_wrap("html", "parse_fragment", html::parse_fragment)?;
    linker.func_wrap("html", "escape", html::escape)?;
    linker.func_wrap("html", "unescape", html::unescape)?;
    linker.func_wrap("html", "kind", html::kind)?;
    linker.func_wrap("html", "child_nodes", html::child_nodes)?;
    linker.func_wrap("html", "has_attr", html::has_attr)?;
    linker.func_wrap("html", "set_attr", html::set_attr)?;
    linker.func_wrap("html", "remove_attr", html::remove_attr)?;
    linker.func_wrap("html", "set_text", html::set_text)?;
    linker.func_wrap("html", "set_html", html::set_html)?;
    linker.func_wrap("html", "prepend", html::prepend)?;
    linker.func_wrap("html", "append", html::append)?;
    linker.func_wrap("html", "children", html::children)?;
    linker.func_wrap("html", "base_uri", html::base_uri)?;
    linker.func_wrap("html", "own_text", html::own_text)?;
    linker.func_wrap("html", "data", html::data)?;
    linker.func_wrap("html", "id", html::id)?;
    linker.func_wrap("html", "tag_name", html::tag_name)?;
    linker.func_wrap("html", "class_name", html::class_name)?;
    linker.func_wrap("html", "has_class", html::has_class)?;
    linker.func_wrap("html", "add_class", html::add_class)?;
    linker.func_wrap("html", "remove_class", html::remove_class)?;
    linker.func_wrap("html", "parent", html::parent)?;
    linker.func_wrap("html", "siblings", html::siblings)?;
    linker.func_wrap("html", "next", html::next)?;
    linker.func_wrap("html", "previous", html::previous)?;
    linker.func_wrap("html", "first", html::first)?;
    linker.func_wrap("html", "last", html::last)?;
    linker.func_wrap("html", "get", html::get)?;
    linker.func_wrap("html", "size", html::size)?;
    linker.func_wrap("html", "attr", html::attr)?;
    linker.func_wrap("html", "outer_html", html::outer_html)?;
    linker.func_wrap("html", "remove", html::remove)?;
    linker.func_wrap("html", "select", html::select)?;
    linker.func_wrap("html", "select_first", html::select_first)?;
    linker.func_wrap("html", "text", html::text)?;
    linker.func_wrap("html", "untrimmed_text", html::untrimmed_text)?;
    linker.func_wrap("html", "html", html::html)?;

    register_stub_imports(linker)?;

    Ok(())
}

/// The JS and canvas modules are not implemented. Sources that only import them
/// for optional functionality (login web views or image processing) can still be
/// instantiated and used for searching and reading.
fn register_stub_imports(linker: &mut Linker<WasmStore>) -> Result<()> {
    macro_rules! stub {
        ($module:literal, $name:literal, ($($arg:ty),*) -> $ret:ty, $value:expr) => {
            linker.func_wrap(
                $module,
                $name,
                |_caller: Caller<'_, WasmStore>, $(_: $arg),*| -> $ret { $value },
            )?;
        };
        ($module:literal, $name:literal, ($($arg:ty),*), $value:expr) => {
            linker.func_wrap(
                $module,
                $name,
                |_caller: Caller<'_, WasmStore>, $(_: $arg),*| { $value },
            )?;
        };
    }

    stub!("js", "context_create", () -> i32, -2);
    stub!("js", "context_eval", (i32, i32, i32) -> i32, -2);
    stub!("js", "context_eval_async", (i32, i32, i32) -> i32, -2);
    stub!("js", "context_get", (i32, i32, i32) -> i32, -2);
    stub!("js", "webview_create", () -> i32, -1);
    stub!("js", "webview_set_rule_list", (i32, i32, i32) -> i32, -1);
    stub!("js", "webview_load", (i32, i32) -> i32, -1);
    stub!("js", "webview_load_html", (i32, i32, i32, i32, i32) -> i32, -1);
    stub!("js", "webview_wait_for_load", (i32) -> i32, -1);
    stub!("js", "webview_eval", (i32, i32, i32) -> i32, -1);
    stub!("js", "webview_eval_async", (i32, i32, i32) -> i32, -1);
    stub!(
        "js",
        "webview_add_user_script",
        (i32, i32, i32, i32, i32) -> i32,
        -1
    );
    stub!("js", "webview_get_cookies", (i32) -> i32, -1);
    stub!(
        "js",
        "webview_delete_cookie",
        (i32, i32, i32, i32, i32, i32, i32) -> i32,
        -1
    );

    stub!("canvas", "new_context", (f32, f32) -> i32, -1);
    stub!(
        "canvas",
        "set_transform",
        (i32, f32, f32, f32, f32, f32) -> i32,
        -1
    );
    stub!(
        "canvas",
        "copy_image",
        (i32, i32, f32, f32, f32, f32, f32, f32, f32, f32) -> i32,
        -1
    );
    stub!(
        "canvas",
        "draw_image",
        (i32, i32, f32, f32, f32, f32) -> i32,
        -1
    );
    stub!("canvas", "fill", (i32, i32, f32, f32, f32, f32) -> i32, -1);
    stub!("canvas", "stroke", (i32, i32, i32) -> i32, -1);
    stub!(
        "canvas",
        "draw_text",
        (i32, i32, i32, f32, f32, f32, i32, f32, f32, f32, f32) -> i32,
        -1
    );
    stub!("canvas", "get_image", (i32) -> i32, -1);
    stub!("canvas", "new_font", (i32, i32) -> i32, -1);
    stub!("canvas", "system_font", (i32) -> i32, -1);
    stub!("canvas", "load_font", (i32, i32) -> i32, -1);
    stub!("canvas", "new_image", (i32, i32) -> i32, -1);
    stub!("canvas", "get_image_data", (i32) -> i32, -1);
    stub!("canvas", "get_image_width", (i32) -> f32, -1.0);
    stub!("canvas", "get_image_height", (i32) -> f32, -1.0);

    Ok(())
}
