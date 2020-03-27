use crate::dom::{NodeType, Document, load_doc_from_bytestring};
use crate::style::{StyledNode, Display, style_tree};
use crate::css::{Color, Unit, Value, parse_stylesheet_from_bytestring, Stylesheet};
use crate::layout::BoxType::{BlockNode, InlineNode, AnonymousBlock, InlineBlockNode, TableNode, TableRowGroupNode, TableRowNode, TableCellNode};
use crate::css::Value::{Keyword, Length};
use crate::css::Unit::Px;
use crate::render::{BLACK, FontCache};
use crate::image::{LoadedImage};
use crate::dom::NodeType::{Text, Element};
use crate::net::{load_image, load_stylesheet_from_net, relative_filepath_to_url, load_doc_from_net, BrowserError};
use std::mem;
use crate::style::Display::{TableRowGroup, TableRow};
use glium_glyph::glyph_brush::{Section, rusttype::{Scale, Font}, GlyphBrush};
use glium_glyph::glyph_brush::GlyphCruncher;
use glium_glyph::glyph_brush::rusttype::Rect as GBRect;

#[derive(Clone, Copy, Debug, Default)]
pub struct Dimensions {
    pub content: Rect,
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
}

impl Dimensions {
    fn padding_box(self) -> Rect {
        self.content.expanded_by(self.padding)
    }
    fn border_box(self) -> Rect {
        self.padding_box().expanded_by(self.border)
    }
    fn margin_box(self) -> Rect {
        self.border_box().expanded_by(self.margin)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn with_inset(self, val:f32) -> Rect {
        Rect {
            x: (self.x + val).floor() + 0.5,
            y: (self.y + val).floor() + 0.5,
            width: (self.width - val - val).floor(),
            height: (self.height - val -val).floor(),
        }
    }
    fn expanded_by(self, edge: EdgeSizes) -> Rect {
        Rect {
            x: self.x - edge.left,
            y: self.y - edge.top,
            width: self.width + edge.left + edge.right,
            height: self.height + edge.top + edge.bottom,
        }
    }
    pub fn contains(self, x:f32, y:f32) -> bool {
        self.x <= x && self.x + self.width >= x && self.y <= y && self.y + self.height > y
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeSizes {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug)]
pub struct LayoutBox<'a> {
    pub dimensions: Dimensions,
    pub box_type: BoxType<'a>,
    pub children: Vec<LayoutBox<'a>>,
}

#[derive(Debug)]
pub enum BoxType<'a> {
    BlockNode(&'a StyledNode<'a>),
    InlineNode(&'a StyledNode<'a>),
    InlineBlockNode(&'a StyledNode<'a>),
    AnonymousBlock(&'a StyledNode<'a>),
    TableNode(&'a StyledNode<'a>),
    TableRowGroupNode(&'a StyledNode<'a>),
    TableRowNode(&'a StyledNode<'a>),
    TableCellNode(&'a StyledNode<'a>),
}

#[derive(Debug)]
pub enum RenderBox {
    Block(RenderBlockBox),
    Anonymous(RenderAnonymousBox),
    Inline(),
    InlineBlock(),
}

#[derive(Debug)]
pub enum QueryResult<'a> {
    Text(&'a RenderTextBox),
    None(),
}
impl QueryResult<'_> {
    fn is_none(&self) -> bool {
        match self {
            QueryResult::None() =>true,
            _ => false
        }
    }
}


impl RenderBox {
    pub fn find_box_containing(&self, x:f32, y:f32) -> QueryResult {
        match self {
            RenderBox::Block(bx) => bx.find_box_containing(x,y),
            RenderBox::Anonymous(bx) => bx.find_box_containing(x,y),
            _ => QueryResult::None(),
        }
    }
}

#[derive(Debug)]
pub struct RenderBlockBox {
    pub title: String,
    pub rect:Rect,
    pub margin:EdgeSizes,
    pub padding:EdgeSizes,
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: EdgeSizes,
    pub valign:String,
    pub children: Vec<RenderBox>,
}

impl RenderBlockBox {
    pub fn find_box_containing(&self, x: f32, y: f32) -> QueryResult {
        for child in self.children.iter() {
            let res = child.find_box_containing(x,y);
            if !res.is_none() {
                return res
            }
        }
        QueryResult::None()
    }
    pub fn content_area_as_rect(&self) -> Rect {
        Rect {
            x: self.rect.x - self.padding.left - self.border_width.left,
            y: self.rect.y - self.padding.top - self.border_width.top,
            width: self.rect.width + self.padding.left + self.padding.right + self.border_width.left + self.border_width.right,
            height: self.rect.height + self.padding.top + self.padding.bottom + self.border_width.left + self.border_width.right,
        }
    }
}

#[derive(Debug)]
pub struct RenderAnonymousBox {
    pub(crate) rect:Rect,
    pub children: Vec<RenderLineBox>,
}
impl RenderAnonymousBox {
    pub fn find_box_containing(&self, x: f32, y: f32) -> QueryResult {
        for child in self.children.iter() {
            let res = child.find_box_containing(x,y);
            if !res.is_none() {
                return res
            }
        }
        QueryResult::None()
    }
}
#[derive(Debug)]
pub struct RenderLineBox {
    pub(crate) rect:Rect,
    pub children: Vec<RenderInlineBoxType>,
    pub(crate) baseline:f32,
}
impl RenderLineBox {
    pub fn find_box_containing(&self, x: f32, y: f32) -> QueryResult {
        for child in self.children.iter() {
            let res = match child {
                RenderInlineBoxType::Text(node) => node.find_box_containing(x,y),
                _ => QueryResult::None()
            };
            if !res.is_none() {
                return res
            }
        }
        QueryResult::None()
    }
}

#[derive(Debug)]
pub enum RenderInlineBoxType {
    Text(RenderTextBox),
    Image(RenderImageBox),
    Block(RenderBlockBox),
    Error(RenderErrorBox),
}

#[derive(Debug)]
pub struct RenderTextBox {
    pub rect:Rect,
    pub text:String,
    pub color:Option<Color>,
    pub font_size:f32,
    pub font_family:String,
    pub link:Option<String>,
    pub font_weight:i32,
    pub font_style:String,
    pub valign:String,
}
impl RenderTextBox {
    pub fn find_box_containing(&self, x: f32, y: f32) -> QueryResult {
        if self.rect.contains(x,y) {
            return QueryResult::Text(&self)
        }
        QueryResult::None()
    }
}

#[derive(Debug)]
pub struct RenderImageBox {
    pub rect:Rect,
    pub image:LoadedImage,
    pub valign:String,
}
#[derive(Debug)]
pub struct RenderErrorBox {
    pub rect:Rect,
    pub valign:String,
}

pub fn build_layout_tree<'a>(style_node: &'a StyledNode<'a>, doc:&Document) -> LayoutBox<'a> {
    let mut root = LayoutBox::new(match style_node.display() {
        Display::Block => BlockNode(style_node),
        Display::Inline => InlineNode(style_node),
        Display::InlineBlock => InlineBlockNode(style_node),
        Display::Table => TableNode(style_node),
        Display::TableRowGroup => TableRowGroupNode(style_node),
        Display::TableRow => TableRowNode(style_node),
        Display::TableCell => TableCellNode(style_node),
        Display::None => panic!("Root node has display none.")
    });


    for child in &style_node.children {
        match child.display() {
            Display::Block =>  root.children.push(build_layout_tree(&child, doc)),
            Display::Inline => root.get_inline_container().children.push(build_layout_tree(&child, doc)),
            Display::InlineBlock => root.get_inline_container().children.push(build_layout_tree(&child, doc)),
            Display::Table => root.children.push(build_layout_tree(&child,doc)),
            Display::TableRowGroup => root.children.push(build_layout_tree(&child, doc)),
            Display::TableRow => root.children.push(build_layout_tree(&child,doc)),
            Display::TableCell => root.children.push(build_layout_tree(&child,doc)),
            Display::None => {  },
        }
    }
    root
}

impl<'a> LayoutBox<'a> {
    fn new(box_type: BoxType<'a>) -> LayoutBox<'a> {
        LayoutBox {
            box_type,
            dimensions: Default::default(),
            children: Vec::new(),
        }
    }
    fn get_style_node(&self) -> &'a StyledNode<'a> {
        match self.box_type {
            BlockNode(node)
            | TableNode(node)
            | TableRowGroupNode(node)
            | TableRowNode(node)
            | TableCellNode(node)
            | InlineNode(node)
            | InlineBlockNode(node)
            | AnonymousBlock(node) => node
        }
    }

    fn get_inline_container(&mut self) -> &mut LayoutBox<'a> {
        match self.box_type {
            InlineNode(_) | InlineBlockNode(_) | AnonymousBlock(_) | TableCellNode(_)=> self,
            BlockNode(node)
            | TableNode(node)
            | TableRowGroupNode(node)
            | TableRowNode(node) => {
                // if last child is anonymous block, keep using it
                match self.children.last() {
                    Some(&LayoutBox { box_type: AnonymousBlock(_node), ..}) => {},
                    _ => self.children.push(LayoutBox::new(AnonymousBlock(node))),
                }
                self.children.last_mut().unwrap()
            }
        }
    }

    pub fn layout(&mut self, containing: &mut Dimensions, font:&mut FontCache, doc:&Document) -> RenderBox {
        match self.box_type {
            BlockNode(_node) =>         RenderBox::Block(self.layout_block(containing, font, doc)),
            TableNode(_node) =>         RenderBox::Block(self.layout_block(containing, font, doc)),
            TableRowGroupNode(_node) => RenderBox::Block(self.layout_block(containing, font, doc)),
            TableRowNode(_node) =>      RenderBox::Block(self.layout_table_row(containing, font, doc)),
            TableCellNode(_node) =>     RenderBox::Anonymous(self.layout_anonymous_2(containing, font, doc)),
            InlineNode(_node) =>        RenderBox::Inline(),
            InlineBlockNode(_node) =>   RenderBox::InlineBlock(),
            AnonymousBlock(_node) =>    RenderBox::Anonymous(self.layout_anonymous_2(containing, font, doc)),
        }
    }
    fn debug_calculate_element_name(&mut self) -> String{
        match self.box_type {
            BlockNode(sn)
            | TableNode(sn)
            | TableRowGroupNode(sn)
            | TableRowNode(sn)
            | TableCellNode(sn)
            => match &sn.node.node_type {
                NodeType::Element(data) => data.tag_name.clone(),
                _ => "non-element".to_string(),
            }
            _ => "non-element".to_string(),
        }
    }
    fn layout_block(&mut self, containing_block: &mut Dimensions, font_cache:&mut FontCache, doc:&Document) -> RenderBlockBox {
        self.calculate_block_width(containing_block);
        self.calculate_block_position(containing_block);
        let children:Vec<RenderBox> = self.layout_block_children(font_cache, doc);
        self.calculate_block_height();
        let zero = Length(0.0, Px);
        let style = self.get_style_node();
        // println!("border top for block is {} {:#?}", self.debug_calculate_element_name(), &style.lookup("border-top", "border-width", &zero));
        RenderBlockBox{
            rect:self.dimensions.content,
            margin: self.dimensions.margin,
            padding: self.dimensions.padding,
            children,
            title: self.debug_calculate_element_name(),
            background_color: self.get_style_node().color("background-color"),
            border_width: EdgeSizes {
                top: self.length_to_px(&style.lookup("border-width-top", "border-width", &zero)),
                bottom: self.length_to_px(&style.lookup("border-width-bottom", "border-width", &zero)),
                left: self.length_to_px(&style.lookup("border-width-top", "border-width", &zero)),
                right: self.length_to_px(&style.lookup("border-width-bottom", "border-width", &zero)),
            },
            border_color: self.get_style_node().color("border-color"),
            valign: String::from("baseline"),
        }
    }

    fn layout_table_row(&mut self, cb:&mut Dimensions, font_cache:&mut FontCache, doc: &Document) -> RenderBlockBox {
        // println!("layout_table_row");
        self.calculate_block_width(cb);
        self.calculate_block_position(cb);
        self.dimensions.content.height = 50.0;
        let mut children:Vec<RenderBox> = vec![];

        // println!("table row dims now {:#?}", self.dimensions);
        //count the number of table cell children
        let mut count = 0;
        for child in self.children.iter() {
            match child.box_type {
                BoxType::TableCellNode(_) => count+=1,
                _ => {}
            }
        }
        let child_width = self.dimensions.content.width / count as f32;
        let self_height = self.dimensions.content.height;
        let mut index = 0;
        for child in self.children.iter_mut() {
            match child.box_type {
                BoxType::TableCellNode(_) => {
                    let mut cb = Dimensions {
                        content: Rect {
                            x: self.dimensions.content.x + child_width * (index as f32),
                            y: self.dimensions.content.y,
                            width: child_width,
                            height: 0.0
                        },
                        padding: Default::default(),
                        border: Default::default(),
                        margin: Default::default()
                    };
                    // println!("table cell child with count {} w = {} index = {} cb = {:#?}",count, child_width,index, cb);
                    let bx = child.layout(&mut cb, font_cache, doc);
                    // println!("table cell child created {:#?}",bx);
                    children.push(bx)
                }
                BoxType::AnonymousBlock(_)=>println!(" anonymous child"),
                _ => {
                    println!("table_row can't have child of {:#?}",child.get_type());
                }
            };
            index += 1;
        };
        let zero = Length(0.0, Px);
        let style = self.get_style_node();
        RenderBlockBox {
            title: self.debug_calculate_element_name(),
            rect:self.dimensions.content,
            margin: self.dimensions.margin,
            padding: self.dimensions.padding,
            background_color: self.get_style_node().color("background-color"),
            border_width: EdgeSizes {
                top: self.length_to_px(&style.lookup("border-top", "border-width", &zero)),
                bottom: self.length_to_px(&style.lookup("border-bottom", "border-width", &zero)),
                left: self.length_to_px(&style.lookup("border-top", "border-width", &zero)),
                right: self.length_to_px(&style.lookup("border-bottom", "border-width", &zero)),
            },
            border_color: self.get_style_node().color("border-color"),
            valign: String::from("baseline"),
            children: children,
        }
    }

    fn find_font_family(&self, looper:&mut Looper) -> String {
        let font_family_values = looper.style_node.lookup(
            "font-family",
            "font-family",
            &Value::Keyword(String::from("sans-serif")));
        // println!("font family values: {:#?} {:#?}",font_family_values, looper.style_node);
        match font_family_values {
            Value::ArrayValue(vals ) => {
                for val in vals.iter() {
                    match val {
                        Value::StringLiteral(str) => {
                            if looper.font_cache.has_font_family(str) {
                                return String::from(str);
                            }
                        }
                        Value::Keyword(str) => {
                            if looper.font_cache.has_font_family(str) {
                                return String::from(str);
                            }
                        }
                        _ => {}
                    }
                }
                println!("no valid font found in stack: {:#?}",vals);
                String::from("sans-serif")
            }
            Value::Keyword(str) => str,
            _ => String::from("sans-serif"),
        }
    }

    fn get_type(&self) -> String {
        match self.box_type {
            BoxType::AnonymousBlock(styled)
            | BoxType::BlockNode(styled)
            | BoxType::TableNode(styled)
            | BoxType::TableRowGroupNode(styled)
            | BoxType::TableRowNode(styled)
            | BoxType::TableCellNode(styled)
            | BoxType::InlineBlockNode(styled)
            | BoxType::InlineNode(styled) => format!("{:#?}",styled.node.node_type)
        }
    }

    fn layout_anonymous_2(&mut self, dim:&mut Dimensions, font_cache:&mut FontCache, doc:&Document) -> RenderAnonymousBox {
        // println!("parent is {:#?}",self.get_type());
        // println!("parent style node is {:#?}",self.get_style_node());
        let mut looper = Looper {
            lines: vec![],
            current: RenderLineBox {
                rect: Rect{
                    x: dim.content.x,
                    y: dim.content.y + dim.content.height,
                    width: dim.content.width,
                    height: 0.0,
                },
                baseline:0.0,
                children: vec![]
            },
            extents: Rect {
                x: dim.content.x,
                y: dim.content.y + dim.content.height,
                width: dim.content.width,
                height: 0.0,
            },
            current_start: dim.content.x,
            current_end: dim.content.x,
            current_bottom: dim.content.y + dim.content.height,
            font_cache:font_cache,
            doc,
            style_node:self.get_style_node(),
        };
        for child in self.children.iter_mut() {
            // println!("working on child {:#?}", child.get_type());
            // println!("current start and end is {} {} ",looper.current_start, looper.current_end);
            match child.box_type {
                InlineBlockNode(_styled) => child.do_inline_block(&mut looper),
                InlineNode(_styled) => child.do_inline(&mut looper),
                _ => println!("cant do this child of an anonymous box"),
            }
            // println!("and now after it is {} {}", looper.current_start, looper.current_end)
        }
        looper.adjust_current_line_vertical();
        let old = looper.current;
        looper.current_bottom += old.rect.height;
        looper.extents.height += old.rect.height;
        looper.lines.push(old);
        self.dimensions.content.y = looper.extents.y;
        self.dimensions.content.width = looper.extents.width;
        self.dimensions.content.height = looper.current_bottom - looper.extents.y ;
        // println!("at the end of the looper, bottom = {} y = {} h = {}",
        //          looper.current_bottom, self.dimensions.content.y, self.dimensions.content.height);
        // println!("line boxes are");
        // for line in looper.lines.iter() {
        //     println!("  line {:#?}",line.rect);
        // }
        return RenderAnonymousBox {
            rect: looper.extents,
            children: looper.lines,
        }
    }

    fn do_inline_block(&mut self, looper:&mut Looper) {
        let mut image_size = Rect { x:0.0, y:0.0, width: 30.0, height:30.0};
        let mut src = String::from("");
        // let w = 100.0;
        if let InlineBlockNode(styled) = self.box_type {
            if let Element(data) = &styled.node.node_type {
                match data.tag_name.as_str() {
                    "img" => {
                        let width = if data.attributes.contains_key("width") {
                            data.attributes.get("width").unwrap().parse::<u32>().unwrap()
                        } else {
                            100
                        };
                        image_size.width = width as f32;
                        let height = if data.attributes.contains_key("height") {
                            data.attributes.get("height").unwrap().parse::<u32>().unwrap()
                        } else {
                            100
                        };
                        image_size.height = height as f32;
                        src = data.attributes.get("src").unwrap().clone();
                    },
                    "button" => {
                        // let font_family = self.find_font_family(looper.font_cache);
                        let font_family = "sans-serif";
                        let font_weight = self.get_style_node().lookup_font_weight(400);
                        let font_size = self.get_style_node().lookup_length_px("font-size", 10.0);
                        let font_style = self.get_style_node().lookup_string("font-style", "normal");
                        println!("button font size is {}",font_size);
                        // let font = looper.font_cache.get_font(&font_family, font_weight, &font_style);
                        let text_node = styled.children[0].node;
                        let text = match &text_node.node_type {
                            NodeType::Text(str) => str,
                            _ => panic!("can't do inline block layout if child isn't text"),
                        };
                        let w: f32 = calculate_word_length(text, looper.font_cache, font_size, &font_family, font_weight, &font_style);
                        println!("calculated width is {}",w);
                        looper.current_end += w;
                        let mut containing_block = Dimensions {
                            content: Rect {
                                x: 0.0,
                                y: 0.0,
                                width: 50.0,
                                height: 0.0,
                            },
                            padding: Default::default(),
                            border: Default::default(),
                            margin: Default::default()
                        };
                        let mut block = self.layout_block(&mut containing_block, looper.font_cache, looper.doc);
                        block.rect.x = looper.current_start;
                        block.rect.y = looper.current.rect.y;
                        block.valign = self.get_style_node().lookup_string("vertical-align","baseline");
                        let rbx = RenderInlineBoxType::Block(block);
                        looper.add_box_to_current_line(rbx);
                        return;
                    },
                    _ => {
                        panic!("We don't handle inline-block on non-images yet: tag_name={}",data.tag_name);
                    },
                }
            }
        }

        let bx = match load_image(looper.doc, &src) {
            Ok(image) => {
                println!("Loaded the image {} {}", image.width, image.height);
                RenderInlineBoxType::Image(RenderImageBox {
                    rect: Rect {
                        x:looper.current_start,
                        y: looper.current.rect.y,
                        width: image.width as f32,
                        height: image.height as f32,
                    },
                    valign: self.get_style_node().lookup_string("vertical-align","baseline"),
                    image
                })
            },
            Err(err) => {
                println!("error loading the image for {} : {:#?}", src, err);
                RenderInlineBoxType::Error(RenderErrorBox {
                    rect: Rect {
                        x:looper.current_start,
                        y: looper.current.rect.y,
                        width: image_size.width,
                        height: image_size.height,
                    },
                    valign: self.get_style_node().lookup_string("vertical-align","baseline"),
                })
            }
        };
        if looper.current_end + image_size.width > looper.extents.width {
            looper.adjust_current_line_vertical();
            looper.start_new_line();
            looper.add_box_to_current_line(bx);
        } else {
            looper.current_end += image_size.width;
            looper.add_box_to_current_line(bx);
        }
    }

    fn do_inline(&self, looper:&mut Looper) {
        let link:Option<String> = match &looper.style_node.node.node_type {
            Text(_) => None,
            NodeType::Comment(_) => None,
            NodeType::Cdata(_) => None,
            Element(ed) => {
                if ed.tag_name == "a" {
                    ed.attributes.get("href").map(|s|String::from(s))
                } else {
                    None
                }
            },
            NodeType::Meta(_) => None,
        };
        if let BoxType::InlineNode(snode) = self.box_type {
            match &snode.node.node_type {
                 NodeType::Text(txt) => {
                    let font_family = self.find_font_family(looper);
                     // println!("using font family {}", font_family);
                    let font_weight = looper.style_node.lookup_font_weight(400);
                    let font_size = looper.style_node.lookup_length_px("font-size", 10.0);
                    let font_style = looper.style_node.lookup_string("font-style", "normal");
                    let vertical_align = looper.style_node.lookup_string("vertical-align","baseline");
                    let line_height = font_size*2.0;
                    // let line_height = looper.style_node.lookup_length_px("line-height", line_height);
                    let color = looper.style_node.lookup_color("color", &BLACK);
                    // println!("text has fam={:#?} color={:#?} fs={}", font_family, color, font_size, );
                    // println!("node={:#?}",self.get_style_node());
                    // println!("parent={:#?}", parent.get_style_node());

                    let mut curr_text = String::new();
                    for word in txt.split_whitespace() {
                        let mut word2 = String::from(" ");
                        word2.push_str(word);
                        let w: f32 = calculate_word_length(word2.as_str(), looper.font_cache, font_size, &font_family, font_weight, &font_style);
                        //if it's too long then we need to wrap
                        if looper.current_end + w > looper.extents.width {
                            //add current text to the current line
                            // println!("wrapping: {} cb = {}", curr_text, looper.current_bottom);
                            let bx = RenderInlineBoxType::Text(RenderTextBox{
                                rect: Rect{
                                    x: looper.current_start,
                                    y: looper.current_bottom,
                                    width: looper.current_end - looper.current_start,
                                    height: line_height
                                },
                                text: curr_text,
                                color: Some(color.clone()),
                                font_size,
                                font_family: font_family.clone(),
                                font_style: font_style.clone(),
                                link: link.clone(),
                                font_weight,
                                valign: vertical_align.clone(),
                            });
                            looper.add_box_to_current_line(bx);
                            //make new current text with the current word
                            curr_text = String::new();
                            curr_text.push_str(&word2);
                            curr_text.push_str(" ");
                            looper.current_bottom += looper.current.rect.height;
                            looper.extents.height += looper.current.rect.height;
                            looper.adjust_current_line_vertical();
                            looper.start_new_line();
                            looper.current_end += w;
                        } else {
                            looper.current_end += w;
                            curr_text.push_str(&word2);
                        }
                    }
                    let bx = RenderInlineBoxType::Text(RenderTextBox{
                        rect: Rect {
                            x: looper.current_start,
                            y: looper.current_bottom,
                            width: looper.current_end - looper.current_start,
                            height: line_height,
                        },
                        text: curr_text,
                        color: Some(color.clone()),
                        font_size,
                        font_family,
                        link: link.clone(),
                        font_weight,
                        font_style,
                        valign: vertical_align.clone(),
                    });
                    looper.add_box_to_current_line(bx);
                }
                //     if child is element
                NodeType::Element(_ed) => {
                    for ch in self.children.iter() {
                        ch.do_inline(looper);
                    }
                }
                _ => {}
            }
        }
    }


    /// Calculate the width of a block-level non-replaced element in normal flow.
    ///
    /// http://www.w3.org/TR/CSS2/visudet.html#blockwidth
    ///
    /// Sets the horizontal margin/padding/border dimensions, and the `width`.
    fn calculate_block_width(&mut self, containing:&mut Dimensions) {
        let style = self.get_style_node();

        // 'width' has initial value 'auto'
        let auto = Keyword("auto".to_string());
        let mut width = style.value("width").unwrap_or_else(||auto.clone());
        // println!("width set to {:#?}",width);
        if let Length(per, Unit::Per) = width {
            // println!("its a percentage width {} {}",per,containing.content.width);
            width = Length(containing.content.width*(per/100.0), Px);
        }

        // margin, border, and padding have initial value of 0
        let zero = Length(0.0, Px);
        let mut margin_left = style.lookup("margin-left","margin", &zero);
        let mut margin_right = style.lookup("margin-right","margin", &zero);
        let border_left = style.lookup("border-width-left","border-width", &zero);
        let border_right = style.lookup("border-width-right","border-width", &zero);
        let padding_left = style.lookup("padding-left","padding", &zero);
        let padding_right = style.lookup("padding-right","padding", &zero);

        // If width is not auto and the total is wider than the container, treat auto margins as 0.
        let total = sum([&margin_left, &margin_right, &border_left, &border_right,
            &padding_left, &padding_right, &width].iter().map(|v| self.length_to_px(v)));
        if width != auto && total > containing.content.width {
            if margin_left == auto {
                margin_left = Length(0.0, Px);
            }
            if margin_right == auto {
                margin_right = Length(0.0,Px);
            }
        }

        // Adjust used values so that the above sum equals `containing_block.width`.
        // Each arm of the `match` should increase the total width by exactly `underflow`,
        // and afterward all values should be absolute lengths in px.
        let underflow = containing.content.width - total;
        // println!("underflow = {}",underflow);

        match (width == auto, margin_left == auto, margin_right == auto) {
            (false,false,false) => {
                margin_right = Length(self.length_to_px(&margin_right) + underflow, Px);
            }
            (false,false,true) => { margin_right = Length(underflow, Px); }
            (false,true,false) => { margin_left = Length(underflow, Px); }
            (true, _, _) => {
                if margin_left == auto { margin_left = Length(0.0, Px); }
                if margin_right == auto { margin_right = Length(0.0, Px); }
                if underflow >= 0.0 {
                    width = Length(underflow, Px);
                } else {
                    width = Length(0.0, Px);
                    margin_right = Length(self.length_to_px(&margin_right) + underflow, Px);
                }
            }
            (false, true, true) => {
                margin_left = Length(underflow / 2.0, Px);
                margin_right = Length(underflow / 2.0, Px);
            }
        }
        // println!("width set to {:#?}",width);

        self.dimensions.content.width = self.length_to_px(&width);
        self.dimensions.padding.left = self.length_to_px(&padding_left);
        self.dimensions.padding.right = self.length_to_px(&padding_right);
        self.dimensions.border.left = self.length_to_px(&border_left);
        self.dimensions.border.right = self.length_to_px(&border_right);
        self.dimensions.margin.left = self.length_to_px(&margin_left);
        self.dimensions.margin.right = self.length_to_px(&margin_right);
        // println!("final width is width= {} padding = {} margin: {}",
        //          self.dimensions.content.width,
        //          self.dimensions.padding.left,
        //          self.dimensions.margin.left);
    }

    fn length_to_px(&self, value:&Value) -> f32{
        let font_size = self.get_style_node().lookup_length_px("font-size", 10.0);
        match value {
            Length(v, Unit::Px) => *v,
            Length(v, Unit::Em) => (*v)*font_size,
            Length(v, Unit::Rem) => (*v)*font_size,
            Length(v, Unit::Per) => {
                println!("WARNING: percentage in length_to_px. should have be converted to pixels already");
                0.0
            }
            _ => {0.0}
        }
    }
    fn calculate_block_position(&mut self, containing: &mut Dimensions) {
        let zero = Length(0.0, Px);
        let style = self.get_style_node();
        //println!("caculating block position {:#?} border {:#?}",style, style.lookup("border-width-top","border-width",&zero));
        let margin = EdgeSizes {
            top: self.length_to_px(&style.lookup("margin-top", "margin", &zero)),
            bottom: self.length_to_px(&style.lookup("margin-bottom","margin",&zero)),
            ..(self.dimensions.margin)
        };
        let border = EdgeSizes {
            top: self.length_to_px(&style.lookup("border-width-top", "border-width", &zero)),
            bottom: self.length_to_px(&style.lookup("border-width-bottom","border-width",&zero)),
            ..(self.dimensions.border)
        };
        let padding = EdgeSizes {
            top: self.length_to_px(&style.lookup("padding-top", "padding", &zero)),
            bottom: self.length_to_px(&style.lookup("padding-bottom","padding",&zero)),
            ..(self.dimensions.padding)
        };

        self.dimensions.margin = margin;
        self.dimensions.border = border;
        self.dimensions.padding = padding;
        let d = &mut self.dimensions;
        d.content.x = containing.content.x + d.margin.left + d.border.left + d.padding.left;
        d.content.y = containing.content.height + containing.content.y + d.margin.top + d.border.top + d.padding.top;
    }

    fn layout_block_children(&mut self, font_cache:&mut FontCache, doc:&Document) -> Vec<RenderBox>{
        let d = &mut self.dimensions;
        let mut children:Vec<RenderBox> = vec![];
        for child in self.children.iter_mut() {
            let bx = child.layout(d, font_cache, doc);
            d.content.height += child.dimensions.margin_box().height;
            children.push(bx)
        };
        children
    }

    fn calculate_block_height(&mut self) {
        if let Some(val) = self.get_style_node().value("height") {
            self.dimensions.content.height = self.length_to_px(&val);
        }
    }

}

fn calculate_word_length(text:&str, fc:&mut FontCache, font_size:f32, font_family:&str, font_weight:i32, font_style:&str) -> f32 {
    let scale = Scale::uniform(font_size * 2.0 as f32);
    fc.lookup_font(font_family,font_weight, font_style);
    let sec = Section {
        text,
        scale,
        ..Section::default()
    };
    let glyph_bounds = fc.brush.glyph_bounds(sec);
    match &glyph_bounds {
        Some(rect) => rect.max.x as f32,
        None => 0.0,
    }
}

struct Looper<'a> {
    lines:Vec<RenderLineBox>,
    current: RenderLineBox,
    extents:Rect,
    current_start:f32,
    current_end:f32,
    current_bottom:f32,
    font_cache:&'a mut FontCache,
    doc: &'a Document,
    style_node: &'a StyledNode<'a>,
}

impl Looper<'_> {
    fn start_new_line(&mut self) {
        let old = mem::replace(&mut self.current, RenderLineBox {
            rect: Rect{
                x: self.extents.x,
                y: self.current_bottom,
                width: self.extents.width,
                height: 0.0
            },
            baseline:0.0,
            children: vec![],
        });
        self.lines.push(old);
        self.current_start = self.extents.x;
        self.current_end = self.extents.x;
    }
    fn add_box_to_current_line(&mut self, bx:RenderInlineBoxType) {
        let rect = match &bx {
            RenderInlineBoxType::Text(bx) => &bx.rect,
            RenderInlineBoxType::Error(bx) => &bx.rect,
            RenderInlineBoxType::Image(bx) => &bx.rect,
            RenderInlineBoxType::Block(bx) => &bx.rect,
        };
        self.current.rect.height = self.current.rect.height.max(rect.height);
        self.current.children.push(bx);
        self.current_start = self.current_end;
    }
    fn adjust_current_line_vertical(&mut self) {
        for ch in self.current.children.iter_mut() {
            let (mut rect,mut string) =  match ch {
                RenderInlineBoxType::Text(bx)    => (&mut bx.rect,&bx.valign),
                RenderInlineBoxType::Error(bx)  => (&mut bx.rect,&bx.valign),
                RenderInlineBoxType::Image(bx) => (&mut bx.rect,&bx.valign),
                RenderInlineBoxType::Block(bx)  => (&mut bx.rect,&bx.valign),
            };
            match string.as_str() {
                "bottom" => {
                    rect.y = self.current.rect.y + self.current.rect.height - rect.height;
                },
                "sub" => {
                    rect.y = self.current.rect.y + self.current.rect.height - rect.height - 10.0 + 10.0;
                },
                "baseline" => {
                    rect.y = self.current.rect.y + self.current.rect.height - rect.height - 10.0;
                },
                "super" => {
                    rect.y = self.current.rect.y + self.current.rect.height - rect.height - 10.0 - 10.0;
                },
                "middle" => {
                    rect.y = self.current.rect.y + (self.current.rect.height - rect.height)/2.0;
                },
                "top" => {
                    rect.y = self.current.rect.y;
                },
                _ => {}
            }
        }
    }

}

/*
#[test]
fn test_layout<'a>() {
    let mut font_cache = FontCache::new();
    font_cache.install_font("sans-serif",400.0, "normal",
                            &relative_filepath_to_url("tests/fonts/Open_Sans/OpenSans-Regular.ttf").unwrap());
    font_cache.install_font("sans-serif", 700.0, "normal",
                            &relative_filepath_to_url("tests/fonts/Open_Sans/OpenSans-Bold.ttf").unwrap());

    let doc = load_doc_from_net(&relative_filepath_to_url("tests/nested.html").unwrap()).unwrap();
    let ss_url = relative_filepath_to_url("tests/default.css").unwrap();
    let mut stylesheet = load_stylesheet_from_net(&ss_url).unwrap();
    font_cache.scan_for_fontface_rules(&stylesheet);
    let snode = style_tree(&doc.root_node,&stylesheet);
    println!(" ======== build layout boxes ========");
    let mut root_box = build_layout_tree(&snode, &doc);
    let mut containing_block = Dimensions {
        content: Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 0.0,
        },
        padding: Default::default(),
        border: Default::default(),
        margin: Default::default()
    };
    // println!("roob box is {:#?}",root_box);
    println!(" ======== layout phase ========");
    let _render_box = root_box.layout(&mut containing_block, &mut font_cache, &doc);
    // println!("final render box is {:#?}", render_box);
}
*/
fn sum<I>(iter: I) -> f32 where I: Iterator<Item=f32> {
    iter.fold(0., |a, b| a + b)
}
/*
#[test]
fn test_inline_block_element_layout() {
    let mut font_cache = FontCache::new();
    font_cache.install_font("sans-serif",400.0, "normal",
                            &relative_filepath_to_url("tests/fonts/Open_Sans/OpenSans-Regular.ttf").unwrap());
    font_cache.install_font("sans-serif", 700.0, "normal",
                            &relative_filepath_to_url("tests/fonts/Open_Sans/OpenSans-Bold.ttf").unwrap());{}

    let doc = load_doc_from_bytestring(b"<html><body><div><button>foofoo</button></div></body></html>");
    let ss_url = relative_filepath_to_url("tests/default.css").unwrap();
    let mut stylesheet = load_stylesheet_from_net(&ss_url).unwrap();
    font_cache.scan_for_fontface_rules(&stylesheet);
    let snode = style_tree(&doc.root_node,&stylesheet);
    let mut root_box = build_layout_tree(&snode, &doc);
    let mut containing_block = Dimensions {
        content: Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 0.0,
        },
        padding: Default::default(),
        border: Default::default(),
        margin: Default::default()
    };
    // println!("roob box is {:#?}",root_box);
    println!(" ======== layout phase ========");
    let _render_box = root_box.layout(&mut containing_block, &mut font_cache, &doc);
}
*/
/*
fn standard_init<'a,R:Resources,F:Factory<R>>(html:&[u8], css:&[u8]) -> (FontCache, Document, Stylesheet){
    let mut font_cache = FontCache::new();
    font_cache.install_font("sans-serif",400.0, "normal",
                            &relative_filepath_to_url("tests/fonts/Open_Sans/OpenSans-Regular.ttf").unwrap());
    font_cache.install_font("sans-serif", 700.0, "normal",
                            &relative_filepath_to_url("tests/fonts/Open_Sans/OpenSans-Bold.ttf").unwrap());{}
    let mut doc = load_doc_from_bytestring(html);
    let stylesheet = parse_stylesheet_from_bytestring(css).unwrap();
    let styled = style_tree(&doc.root_node,&stylesheet);
    let mut root_box = build_layout_tree(&styled, &doc);
    let mut cb = Dimensions {
        content: Rect {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 0.0,
        },
        padding: Default::default(),
        border: Default::default(),
        margin: Default::default()
    };
    let render_box = root_box.layout(&mut cb, &mut font_cache, &doc);
    // println!("the final render box is {:#?}",render_box);
    return (font_cache,doc, stylesheet);
}
*/
/*
#[test]
fn test_table_layout() {
    let render_box = standard_init(
        br#"<table>
            <tbody>
                <tr>
                    <td>data 1</td>
                    <td>data 2</td>
                    <td>data 3</td>
                </tr>
                <tr>
                    <td>data 4</td>
                    <td>data 5</td>
                    <td>data 6</td>
                </tr>
                <tr>
                    <td>data 7</td>
                    <td>data 8</td>
                    <td>data 9</td>
                </tr>
            </tbody>
        </table>"#,
        br#"
        table {
            display: table;
        }
        tbody {
            display: table-row-group;
        }
        tr {
            display: table-row;
        }
        td {
            display: table-cell;
        }
        "#
    );
    println!("it all ran! {:#?}",render_box);
}
*/

pub enum Brush {
    Style1(glium_glyph::GlyphBrush<'static, 'static>),
    Style2(glium_glyph::glyph_brush::GlyphBrush<'static, Font<'static>>),
}
impl Brush {
    fn glyph_bounds(&mut self, sec:Section) -> Option<GBRect<f32>> {
        match self {
            Brush::Style1(b) => b.glyph_bounds(sec),
            Brush::Style2(b) => b.glyph_bounds(sec),
        }
    }
    pub fn queue(&mut self, sec:Section) {
        match self {
            Brush::Style1(b) => b.queue(sec),
            Brush::Style2(b) => b.queue(sec),
        }
    }
    pub fn draw_queued_with_transform(&mut self, mat:[[f32;4];4],
                                      facade:&glium::Display,
                                      frame:&mut glium::Frame) {
        match self {
            Brush::Style1(b) => b.draw_queued_with_transform(mat,facade,frame),
            Brush::Style2(b) => {
                panic!("cant actuually draw with style two")
            },
        }
    }
}

fn standard_init(html:&[u8],css:&[u8]) -> Result<RenderBox,BrowserError> {

    let open_sans_light: &[u8] = include_bytes!("../tests/fonts/Open_Sans/OpenSans-Light.ttf");
    let open_sans_reg: &[u8] = include_bytes!("../tests/fonts/Open_Sans/OpenSans-Regular.ttf");
    let open_sans_bold: &[u8] = include_bytes!("../tests/fonts/Open_Sans/OpenSans-Bold.ttf");
    let doc = load_doc_from_bytestring(html);
    let stylesheet = parse_stylesheet_from_bytestring(css).unwrap();
    let styled = style_tree(&doc.root_node,&stylesheet);
    let mut glyph_brush:glium_glyph::glyph_brush::GlyphBrush<Font> =
        glium_glyph::glyph_brush::GlyphBrushBuilder::without_fonts().build();
    let mut viewport = Dimensions {
        content: Rect {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 0.0,
        },
        padding: Default::default(),
        border: Default::default(),
        margin: Default::default()
    };
    let mut root_box = build_layout_tree(&styled, &doc);
    let mut font_cache = FontCache {
        brush: Brush::Style2(glyph_brush),
        families: Default::default(),
        fonts: Default::default()
    };
    font_cache.install_font(Font::from_bytes(open_sans_light)?,"sans-serif",100, "normal");
    font_cache.install_font(Font::from_bytes(open_sans_reg)?,"sans-serif",400, "normal");
    font_cache.install_font(Font::from_bytes(open_sans_bold)?,"sans-serif",700, "normal");
    let render_box = root_box.layout(&mut viewport, &mut font_cache, &doc);
    return Ok(render_box);
}

#[test]
fn test_insets() {
    let render_box = standard_init(
        br#"<body></body>"#,
        br#"body { display:block; margin: 50px; padding: 50px; border-width: 50px; } "#
    ).unwrap();
    println!("it all ran! {:#?}",render_box);
    match render_box {
        RenderBox::Block(bx) => {
            assert_eq!(bx.margin.left,50.0);
            assert_eq!(bx.padding.left,50.0);
            assert_eq!(bx.border_width.left,50.0);
        }
        _ => {
            panic!("this should have been a block box");
        }
    }
    // assert_eq!(render_box.calculate_insets().left,100);

}

#[test]
fn test_font_weight() {
    let render_box = standard_init(
        br#"<body>text</body>"#,
        br#"body { display:block; font-weight: bold; } "#
    ).unwrap();
    println!("it all ran! {:#?}",render_box);
}

#[test]
fn test_blue_text() {
    let render_box = standard_init(
        br#"<body><a>link</a></body>"#,
        br#" a { color: blue; } body { display: block; color: red; }"#
    ).unwrap();
    println!("it all ran! {:#?}",render_box);
/*
    match render_box {
        RenderBox::Block(bx) => {
            // bx.children[0].children
            assert_eq!(bx.margin.left,50.0);
            assert_eq!(bx.padding.left,50.0);
            assert_eq!(bx.border_width.left,50.0);
        }
        _ => {
            panic!("this should have been a block box");
        }
    }
    // assert_eq!(render_box.calculate_insets().left,100);
*/
}
