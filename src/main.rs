use rust_minibrowser::dom::{load_doc, getElementsByTagName, NodeType, Document};
use rust_minibrowser::style;
use rust_minibrowser::layout;

use minifb::{Window, WindowOptions, MouseButton, MouseMode, KeyRepeat, Key};
use raqote::{DrawTarget, SolidSource, Source, Point, Transform};
use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use rust_minibrowser::style::style_tree;
use rust_minibrowser::css::{parse_stylesheet, Stylesheet};
use rust_minibrowser::layout::{Dimensions, Rect, RenderBox, QueryResult};
use rust_minibrowser::render::draw_render_box;
use rust_minibrowser::net::{load_doc_from_net, load_stylesheet_with_fallback, relative_filepath_to_url, calculate_url_from_doc, BrowserError};
use rust_minibrowser::globals::make_globals;
use std::env::current_dir;
use std::path::{PathBuf, Path};
use url::Url;
use font_kit::font::Font;


const WIDTH: usize = 600;
const HEIGHT: usize = 1100;


fn navigate_to_doc(url:Url, font:&Font, containing_block:Dimensions) -> Result<(Document, RenderBox),BrowserError> {
    let doc = load_doc_from_net(&url)?;
    let stylesheet = load_stylesheet_with_fallback(&doc)?;
    let styled = style_tree(&doc.root_node,&stylesheet);
    let mut bbox = layout::build_layout_tree(&styled, &doc);
    let render_root = bbox.layout(containing_block, &font, &doc);
    return Ok((doc,render_root))
}

fn main() -> Result<(),BrowserError>{
    let globals = make_globals();
    let mut window = Window::new("Rust-Minibrowser", WIDTH, HEIGHT, WindowOptions {
        ..WindowOptions::default()
    }).unwrap();
    let font = SystemSource::new()
        .select_best_match(&[FamilyName::SansSerif], &Properties::new())
        .unwrap()
        .load()
        .unwrap();

    let size = window.get_size();
    let size = Rect {
        x: 0.0,
        y: 0.0,
        width: size.0 as f32,
        height: size.1 as f32,
    };

    // let doc = load_doc_from_net("https://apps.josh.earth/rust-minibrowser/test1.html").unwrap();

    // let doc = load_doc("tests/simple.html");
    // let doc = load_doc("tests/image.html");
    let containing_block = Dimensions {
        content: Rect {
            x: 0.0,
            y: 0.0,
            width: WIDTH as f32,
            height: 0.0,
        },
        padding: Default::default(),
        border: Default::default(),
        margin: Default::default()
    };
    // println!("render root is {:#?}",render_root);

    // let start_page = relative_filepath_to_url("tests/page1.html")?;
    // let start_page = Url::parse("https://apps.josh.earth/rust-minibrowser/test1.html").unwrap();
    // let start_page = Url::parse("https://edwardtufte.github.io/tufte-css/").unwrap();
    let start_page = relative_filepath_to_url("tests/tufte/tufte.html")?;
    let (mut doc, mut render_root) = navigate_to_doc(start_page, &font, containing_block).unwrap();
    let mut dt = DrawTarget::new(size.width as i32, size.height as i32);
    let mut prev_left_down = false;
    let mut viewport = Rect{
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0
    };
    loop {
        scroll_viewport(&window, &mut viewport);
        let ts = Transform::row_major(1.0, 0.0, 0.0, 1.0, viewport.x, viewport.y);
        dt.set_transform(&ts);

        let left_down = window.get_mouse_down(MouseButton::Left);
        if left_down && !prev_left_down {
            let (x,y) = window.get_mouse_pos(MouseMode::Clamp).unwrap();
            println!("Left mouse is down at {} , {}",x,y);
            let res = render_root.find_box_containing(x,y);
            println!("got a result under the click: {:#?}", res);
            match res {
                QueryResult::Text(bx) => {
                    match &bx.link {
                        Some(href) => {
                            let res = navigate_to_doc(calculate_url_from_doc(doc,href).unwrap(), &font, containing_block).unwrap();
                            doc = res.0;
                            render_root = res.1;
                        }
                        _ => {}
                    }
                }

                _ => {}
            }

        }
        prev_left_down = left_down;

        dt.clear(SolidSource::from_unpremultiplied_argb(0xff, 0xff, 0xff, 0xff));
        draw_render_box(&render_root, &mut dt, &font, &viewport);
        window.update_with_buffer(dt.get_data(), size.width as usize, size.height as usize).unwrap();
    }
}

fn scroll_viewport(window:&Window, viewport:&mut Rect) {
    let mut keys = window.get_keys_pressed(KeyRepeat::No);
    println!("keys pressed {:#?}",keys);
    if keys.is_some() {
        match keys {
            Some(keys) => {
                for key in keys {
                    match key {
                        Key::Up => viewport.y += 100.0,
                        Key::Down => viewport.y -= 100.0,
                        Key::Left => viewport.x += 100.0,
                        Key::Right => viewport.x -= 100.0,
                        _ => {}
                    }
                }
            },
            None => {

            },
        };
    }
}
