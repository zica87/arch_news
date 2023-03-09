use anyhow::bail;
use regex::Regex;
use scraper::{Html, Selector};
use serde::Serialize;

const LAST_TITLE_PATH: &str = "last_title.txt";

fn main() {
    if let Err(e) = helper() {
        println!("{e:#?}");
        std::process::exit(1);
    }
}

fn helper() -> Result<(), anyhow::Error> {
    let LAST_TITLE = String::from_utf8(std::fs::read(LAST_TITLE_PATH)?)?;
    let TELEGRAM_BOT_TOKEN = match std::env::var("TELEGRAM_BOT_TOKEN") {
        Ok(v) => v,
        Err(e) => match e {
            std::env::VarError::NotPresent => {
                bail!(r#""TELEGRAM_BOT_TOKEN" env not found"#);
            }
            _ => {
                bail!(e);
            }
        },
    };
    let SEND_MESSAGE_URL = format!("https://api.telegram.org/bot{TELEGRAM_BOT_TOKEN}/sendMessage");

    let news_list_html = reqwest::blocking::get("https://archlinux.org/news/")?.text()?;
    let news_list = Html::parse_document(&news_list_html);

    let mut message_list = Vec::<String>::new();

    let line_break_not_following_a_tag = Regex::new(r"[^>]\n").unwrap();

    let td = Selector::parse("td").unwrap();
    let tr = Selector::parse("tr").unwrap();
    let a = Selector::parse("a").unwrap();
    let tbody_selector = Selector::parse("tbody").unwrap();
    let article_selector = Selector::parse("div.article-content").unwrap();

    let Some(tbody) = news_list.select(&tbody_selector).next()
    else {
        bail!(news_list_html);
    };

    let mut current_latest_title = None;

    for item in tbody.select(&tr) {
        let tds = item.select(&td).collect::<Vec<_>>();
        let Some(td1_a) = tds[1].select(&a).next()
            else {
                bail!(news_list_html);
            };
        let Some(title) = td1_a.text().next()
            else {
                bail!(news_list_html);
            };
        if title == LAST_TITLE {
            break;
        }

        if current_latest_title.is_none() {
            current_latest_title = Some(title);
        }

        let Some(author) = tds[2].text().next()
            else {
                bail!(news_list_html);
            };
        let Some(partial_article_url) = td1_a.value().attr("href")
            else {
                bail!(news_list_html);
            };
        let article_url = "https://archlinux.org".to_string() + partial_article_url;

        let article_html = reqwest::blocking::get(&article_url)?.text()?;
        let article = Html::parse_document(&article_html);
        let Some(article_wrapper) = article.select(&article_selector).next()
            else {
                bail!(article_html);
            };

        let content = article_wrapper.inner_html();
        let line_break_as_space_positions = line_break_not_following_a_tag
            .find_iter(&content)
            .map(|matchh| matchh.end() - 1)
            .collect::<Vec<_>>();
        let mut content_bytes = content.into_bytes();
        line_break_as_space_positions.into_iter().for_each(|p| {
            content_bytes[p] = b' ';
        });
        let c = match String::from_utf8(content_bytes) {
            Ok(c) => c,
            Err(e) => {
                bail!(e);
            }
        };
        let content = c
            .replace("<p>", "")
            .replace("</p>", "\n")
            .replace("<ul>\n", "")
            .replace("</ul>", "")
            .replace("<li>", "- ")
            .replace("</li>", "")
            .replace("<br>", "")
            .replace("</pre>\n", "</pre>\n\n");

        message_list.push(
            format!(r#"<b><a href="{article_url}">{title}</a></b>"#)
                + "\n\n"
                + content.trim()
                + "\n\n\n"
                + author,
        );
    }
    for message in message_list.into_iter().rev() {
        let response = reqwest::blocking::Client::new()
            .post(&SEND_MESSAGE_URL)
            .json(&MessageJson {
                chat_id: "-1001823767670",
                text: message,
                parse_mode: "HTML",
                disable_web_page_preview: true,
            })
            .send()?;
        if let Err(e) = response.error_for_status_ref() {
            if let Ok(t) = response.text() {
                bail!(t);
            } else {
                bail!(e);
            }
        }
    }
    if let Some(t) = current_latest_title {
        std::fs::write(LAST_TITLE_PATH, t)?;
    }

    Ok(())
}

#[derive(Serialize)]
struct MessageJson {
    chat_id: &'static str,
    text: String,
    parse_mode: &'static str,
    disable_web_page_preview: bool,
}
