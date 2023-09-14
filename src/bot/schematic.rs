use anyhow::{anyhow, Result};
use mindus::data::DataRead;
use mindus::*;
use oxipng::*;
use poise::serenity_prelude::*;
use regex::Regex;
use std::sync::LazyLock;
use std::{borrow::Cow, ops::ControlFlow};

use super::{emojis, strip_colors, SMsg, SUCCESS};

static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(```)?(\n)?([^`]+)(\n)?(```)?").unwrap());

async fn from_attachments(attchments: &[Attachment]) -> Result<Option<Schematic>> {
    for a in attchments {
        if a.filename.ends_with("msch") {
            let s = a.download().await?;
            let mut s = DataRead::new(&s);
            let Ok(s) = Schematic::deserialize(&mut s) else {
                continue;
            };
            return Ok(Some(s));
        // discord uploads base64 as a file when its too long
        } else if a.filename == "message.txt" {
            let Ok(s) = String::from_utf8(a.download().await?) else {
                continue;
            };
            let Ok(s) = Schematic::deserialize_base64(&s) else {
                continue;
            };
            return Ok(Some(s));
        }
    }
    Ok(None)
}

pub async fn with(m: SMsg, c: &serenity::client::Context) -> Result<ControlFlow<Message, ()>> {
    let author = m.author;
    let send = |v: Schematic| async move {
        let d = v.tags.get("description").cloned();
        let name = strip_colors(v.tags.get("name").unwrap());
        let cost = v.compute_total_cost().0;
        let p = tokio::task::spawn_blocking(move || to_png(&v)).await?;
        anyhow::Ok(
            m.channel
                .send_message(c, |m| {
                    m.add_file(AttachmentType::Bytes {
                        data: Cow::Owned(p),
                        filename: "image.png".to_string(),
                    })
                    .embed(|e| {
                        e.attachment("image.png");
                        d.map(|v| e.description(v));
                        let mut s = String::new();
                        for (i, n) in cost.iter() {
                            if n == 0 {
                                continue;
                            }
                            use std::fmt::Write;
                            write!(s, "{} {n} ", emojis::item(i)).unwrap();
                        }
                        e.field("", s, true);
                        e.title(name)
                            .footer(|f| f.text(format!("requested by {author}")))
                            .color(SUCCESS)
                    })
                })
                .await?,
        )
    };

    if let Ok(Some(v)) = from_attachments(&m.attachments).await {
        println!("sent {}", v.tags.get("name").unwrap());
        return Ok(ControlFlow::Break(send(v).await?));
    }
    if let Ok(v) = from_msg(&m.content) {
        println!("sent {}", v.tags.get("name").unwrap());
        return Ok(ControlFlow::Break(send(v).await?));
    }
    Ok(ControlFlow::Continue(()))
}

pub fn to_png(s: &Schematic) -> Vec<u8> {
    let p = s.render();
    let p = RawImage::new(
        p.width(),
        p.height(),
        ColorType::RGB {
            transparent_color: None,
        },
        BitDepth::Eight,
        p.take_buffer(),
    )
    .unwrap();
    p.create_optimized_png(&oxipng::Options::default()).unwrap()
}

fn from_msg(msg: &str) -> Result<Schematic> {
    let schem_text = RE
        .captures(msg)
        .ok_or(anyhow!("couldnt find schematic"))?
        .get(3)
        .unwrap()
        .as_str();
    Ok(Schematic::deserialize_base64(schem_text)?)
}
