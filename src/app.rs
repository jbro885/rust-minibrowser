use url::Url;
use crate::render::{FontCache};
use crate::layout::{Dimensions, RenderBox};
use crate::dom::{Document, strip_empty_nodes, expand_entities};
use crate::net::{BrowserError, load_doc_from_net, load_stylesheets_with_fallback, relative_filepath_to_url};
use crate::style::{expand_styles, style_tree};
use crate::layout;
use std::env;
use glium_glyph::glyph_brush::rusttype::Font;

pub fn navigate_to_doc(url:&Url, font_cache:&mut FontCache, containing_block:Dimensions) -> Result<(Document, RenderBox),BrowserError> {
    let mut doc = load_doc_from_net(&url)?;
    strip_empty_nodes(&mut doc);
    expand_entities(&mut doc);
    let mut stylesheet = load_stylesheets_with_fallback(&doc)?;
    expand_styles(&mut stylesheet);
    font_cache.scan_for_fontface_rules(&stylesheet);
    let styled = style_tree(&doc.root_node,&stylesheet);
    // println!("font_cache looks like {:#?}",font_cache.families);
    let mut bbox = layout::build_layout_tree(&styled, &doc);
    // println!("doing layout with bounds {:#?}", containing_block);
    let render_root = bbox.layout(&mut containing_block.clone(), font_cache, &doc);
    // println!("render root is {:#?}",render_root);
    Ok((doc,render_root))
}

pub fn install_standard_fonts(font_cache:&mut FontCache) -> Result<(),BrowserError> {
    let open_sans_light: &[u8] = include_bytes!("../tests/fonts/Open_Sans/OpenSans-Light.ttf");
    let open_sans_reg: &[u8] = include_bytes!("../tests/fonts/Open_Sans/OpenSans-Regular.ttf");
    let open_sans_bold: &[u8] = include_bytes!("../tests/fonts/Open_Sans/OpenSans-Bold.ttf");
    let open_sans_italic:&[u8] = include_bytes!("../tests/fonts/Open_Sans/OpenSans-Italic.ttf");
    let monospace:&[u8] = include_bytes!("../tests/fonts/Source_Code_Pro/SourceCodePro-Regular.ttf");
    let monospace_bold:&[u8] = include_bytes!("../tests/fonts/Source_Code_Pro/SourceCodePro-Bold.ttf");
    font_cache.install_font(Font::from_bytes(open_sans_light)?,
                            "sans-serif",100, "normal");
    font_cache.install_font(Font::from_bytes(open_sans_reg)?,
                            "sans-serif",400, "normal");
    font_cache.install_font(Font::from_bytes(open_sans_bold)?,
                            "sans-serif",700, "normal");
    font_cache.install_font(Font::from_bytes(open_sans_italic)?,
                            "sans-serif",400,"italic");
    font_cache.install_font(Font::from_bytes(monospace)?,
                            "monospace",400,"normal");
    font_cache.install_font(Font::from_bytes(monospace_bold)?,
                            "monospace",700,"normal");
    Ok(())
}

pub fn parse_args() -> Result<Url, BrowserError> {
    let args: Vec<String> = env::args().collect();
    println!("args = {:?}", args);
    let mut start_page = relative_filepath_to_url("tests/page1.html")?;
    if args.len() > 1 {
        println!("loading url {}", args[1]);
        if args[1].starts_with("http") {
            start_page = Url::parse(args[1].as_str())?;
        } else {
            start_page = relative_filepath_to_url(&*args[1])?;
        }
    }
    Ok(start_page)
}
