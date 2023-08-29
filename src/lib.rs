use anyhow::Error;
use mdbook::book::{Book, BookItem, Chapter};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pulldown_cmark::{Event, HeadingLevel, Tag};
use pulldown_cmark_to_cmark::cmark;
use sha2::Digest;
use std::path::PathBuf;

/// A preprocessor to split h1 headings into individual chapters.
#[derive(Default)]
pub struct Split;

fn is_h1(event: &Event) -> bool {
    matches!(event, Event::Start(Tag::Heading(HeadingLevel::H1, _, _)))
}

fn to_cmark(events: Vec<Event>) -> Result<String, Error> {
    let mut buf = String::new();
    cmark(events.into_iter(), &mut buf)?;
    Ok(buf)
}

fn to_chapter(events: Vec<Event>) -> Result<Chapter, Error> {
    let name = &events
        .windows(2)
        .find_map(|window| is_h1(&window[0]).then_some(&window[1]))
        .and_then(|event| match event {
            Event::Text(text) => Some(text.to_string()),
            _ => None,
        })
        .unwrap_or_default();

    let content = to_cmark(events)?;

    let mut hasher = sha2::Sha256::new();
    hasher.update(name);
    let result = hasher.finalize();

    Ok(Chapter {
        name: name.to_string(),
        path: Some(PathBuf::from(format!("{result:x}"))),
        content,
        ..Default::default()
    })
}

fn split_chapter(chapter: &Chapter) -> Result<Vec<Chapter>, Error> {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
    options.insert(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION);
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);

    let parser = pulldown_cmark::Parser::new_ext(&chapter.content, options);
    let mut chapters = vec![];
    let mut events = vec![];

    for event in parser {
        let finish = is_h1(&event) && !events.is_empty();

        if finish {
            chapters.push(to_chapter(events)?);
            events = vec![event];
        } else {
            events.push(event);
        }
    }

    if !events.is_empty() {
        chapters.push(to_chapter(events)?);
    }

    Ok(chapters)
}

impl Preprocessor for Split {
    fn name(&self) -> &str {
        "split"
    }

    fn run(&self, _ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        let mut new_book = Book::new();

        for item in book.sections {
            match item {
                BookItem::Chapter(ref chapter) => {
                    for item in split_chapter(chapter)? {
                        new_book.push_item(item);
                    }
                }
                BookItem::Separator => {
                    new_book.push_item(BookItem::Separator);
                }
                BookItem::PartTitle(title) => {
                    new_book.push_item(BookItem::PartTitle(title));
                }
            }
        }

        Ok(new_book)
    }

    fn supports_renderer(&self, _renderer: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn nop_preprocessor_run() {
        let input_json = r##"[
                {
                    "root": "/path/to/book",
                    "config": {
                        "book": {
                            "authors": ["AUTHOR"],
                            "language": "en",
                            "multilingual": false,
                            "src": "src",
                            "title": "TITLE"
                        },
                        "preprocessor": {
                            "nop": {}
                        }
                    },
                    "renderer": "html",
                    "mdbook_version": "0.4.21"
                },
                {
                    "sections": [
                        {
                            "Chapter": {
                                "name": "Chapter 1",
                                "content": "# Chapter 1\n\n# Chapter 2\n",
                                "number": [1],
                                "sub_items": [],
                                "path": "chapter_1.md",
                                "source_path": "chapter_1.md",
                                "parent_names": []
                            }
                        }
                    ],
                    "__non_exhaustive": null
                }
            ]"##;
        let input_json = input_json.as_bytes();

        let (ctx, book) = mdbook::preprocess::CmdPreprocessor::parse_input(input_json).unwrap();
        let result = Split::default().run(&ctx, book);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.sections.len(), 2);

        let mut iter = processed.sections.iter();

        let chapter_1 = iter.next().unwrap();
        assert!(matches!(chapter_1, BookItem::Chapter(_)));

        match chapter_1 {
            BookItem::Chapter(chapter) => {
                assert_eq!(chapter.name, "Chapter 1");
                assert_eq!(
                    chapter.path.as_ref().unwrap().to_str().unwrap(),
                    "3178a647e0f2bcd284eaa96aab1750e61d3211c14aa60f2b45b6bdd27da6a159"
                );
            }
            _ => {}
        }

        let chapter_2 = iter.next().unwrap();
        assert!(matches!(chapter_2, BookItem::Chapter(_)));

        match chapter_2 {
            BookItem::Chapter(chapter) => {
                assert_eq!(chapter.name, "Chapter 2");
                assert_eq!(
                    chapter.path.as_ref().unwrap().to_str().unwrap(),
                    "11012a8623e958a2b46fc910d209280c789328566b5ab5b3652c71c1ccf7b4fb"
                );
            }
            _ => {}
        }
    }
}
