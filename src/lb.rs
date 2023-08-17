use std::collections::HashMap;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use scraper::{ElementRef, Node};
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

/// entry object
/// 
/// you obtain instances of this object through a Leaderboard,
/// specifically, it's `.from_site` or `.from_cache` methods.
#[derive(Debug)]
pub struct Entry {
    rank: u16,
    name: String,
    user_id: u32,
    run_id: u32,
    score: f32,
}

impl Entry {
    /// reads an Entry out of some async reader
    async fn read(r: &mut (impl io::AsyncRead + Unpin)) -> io::Result<Self> {
        let rank = r.read_u16_le().await?;
        let name = {
            let len = r.read_u8().await?;
            let mut t = vec![0; len as usize];
            r.read_exact(&mut t).await?;
            String::from_utf8(t).unwrap()
        };
        let user_id = r.read_u32_le().await?;
        let run_id = r.read_u32_le().await?;
        let score = r.read_f32_le().await?;

        Ok(Entry {
            rank,
            name,
            user_id,
            run_id,
            score,
        })
    }

    /// writes an Entry into some async reader
    async fn write(&self, w: &mut (impl io::AsyncWrite + Unpin)) -> io::Result<()> {
        w.write_u16_le(self.rank).await?;
        let str = self.name.as_bytes();
        w.write_u8(str.len() as u8).await?;
        w.write_all(str).await?;
        w.write_u32_le(self.user_id).await?;
        w.write_u32_le(self.run_id).await?;
        w.write_f32_le(self.score).await?;

        Ok(())
    }

    /// checks two Entries to see if they have the
    /// same user_id
    pub fn same_user(&self, other: &Self) -> bool {
        self.user_id == other.user_id
    }
}

#[derive(Debug)]
/// represets a whole leaderboard
/// 
/// contains methods to read from/write to a cache
/// or read out from the website.
pub struct Leaderboard {
    timestamp: Duration,
    entries: Vec<Entry>,
}

impl Leaderboard {
    /// Scrape the leaderboard off the site
    pub async fn from_site() -> reqwest::Result<Option<Self>> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        // GET the leaderboard
        let html = reqwest::get("https://hyprd.mn/leaderboards")
            .await?
            .text()
            .await?;

        // use a dom lib to help scrape the doc
        let doc = scraper::Html::parse_document(&html);
        // create a new selector
        let sel = scraper::Selector::parse(".leaderboard>tbody>tr").unwrap();

        // helper function to parse html output
        fn parse_row(row: ElementRef) -> Option<Entry> {
            // u gotta do what u gotta do

            let mut cols = row.children();

            cols.next();

            let Node::Text(rank) = cols.next()?.children().next()?.value() else {
                return None
            };

            let a = cols.next()?.children().next()?;
            let user_url = a.value().as_element()?.attr("href")?;
            let Node::Text(name) = a.children().next()?.value() else {
                return None
            };

            let a = cols.next()?.children().next()?;
            let run_url = a.value().as_element()?.attr("href")?;
            let Node::Text(score) = a.children().next()?.value() else {
                return None
            };

            let entry = Entry {
                rank: rank.parse().ok()?,
                name: name.to_string(),
                user_id: user_url.split('/').last()?.parse().ok()?,
                run_id: run_url.split('/').last()?.parse().ok()?,
                score: score.parse().ok()?,
            };

            Some(entry)
        }

        let Some(entries) = doc
            // use selector
            .select(&sel)
            // every 2nd row (feature of the site :p)
            .step_by(2)
            // map using the helper function
            .map(parse_row)
            // collect into Option<Vec<Entries>>
            // if Option = None, return Ok(None)
            // else entries = Vec<Entries>
            .collect::<Option<Vec<_>>>() else { return Ok(None) };

        Ok(Some(Self { timestamp, entries }))
    }

    /// get a Leaderboard from cache
    pub async fn from_cache() -> io::Result<Self> {
        let mut cache = File::open("cache").await?;
        let mut buf = io::BufReader::new(&mut cache);

        let raw_timestamp = buf.read_u64_le().await?;
        let timestamp = Duration::from_secs(raw_timestamp);

        let mut entries = Vec::new();
        for _ in 0..1000 {
            entries.push(Entry::read(&mut buf).await?);
        }

        Ok(Self { timestamp, entries })
    }

    /// write the Leaderboard to cache
    pub async fn cache(&self) -> io::Result<()> {
        let mut cache = File::create("cache").await?;
        let mut buf = io::BufWriter::new(&mut cache);

        buf.write_u64_le(self.timestamp.as_secs()).await?;

        for entry in 0..1000 {
            self.entries[entry].write(&mut buf).await?;
        }

        buf.flush().await?;

        Ok(())
    }

    pub fn pbs<'a>(&'a self, new: &'a Self) -> Vec<Pb<'a>> {
        Pb::diff(&self.entries, &new.entries)
    }
}

#[derive(Debug)]
pub struct Pb<'a> {
    old: Option<&'a Entry>,
    new: &'a Entry,
}

impl<'a> Pb<'a> {
    fn new(old: Option<&'a Entry>, new: &'a Entry) -> Self {
        Self { old, new }
    }

    pub fn diff(old: &'a Vec<Entry>, new: &'a Vec<Entry>) -> Vec<Self> {
        let mut pbs = Vec::new();
        let mut old: HashMap<_, _> = old.iter().map(|e| (e.user_id, e)).collect();

        for new in new {
            let old = old.remove(&new.user_id);

            if let Some(old) = old {
                if new.run_id == old.run_id {
                    continue;
                }
                pbs.push(Self::new(Some(old), new));
            } else {
                pbs.push(Self::new(None, new));
            }
        }

        pbs
    }
}

impl std::fmt::Display for Pb<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(old) = self.old {
            if self.new.rank == 1 {
                writeln!(f, "---  NEW WORLD RECORD  ---")?;
            } else if self.new.score > 400.0 && 400.0 > old.score {
                writeln!(f, "---  NEW 400  ---")?;
            }

            writeln!(
                f,
                "{} just got a new high score! Score: {} (+{})",
                self.new.name,
                self.new.score,
                self.new.score - old.score
            )?;

            if let Some(sub) = old.rank.checked_sub(self.new.rank) {
                writeln!(
                    f,
                    "They are now rank #{}, gaining {} ranks.",
                    self.new.rank, sub
                )?;
            } else {
                writeln!(f, "They are now rank #{}.", self.new.rank)?;
            }
        } else {
            writeln!(
                f,
                "{} just got a new high score! Score: {}",
                self.new.name, self.new.score
            )?;
            writeln!(f, "They are now rank #{}", self.new.rank)?;
        }
        writeln!(f, "Watch in-game: hyperdemon://run/{}", self.new.run_id)?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pb() {
        let old = Leaderboard {
            timestamp: Duration::from_secs(0),
            entries: vec![
                Entry {
                    rank: 1,
                    name: "possm".to_string(),
                    user_id: 1,
                    run_id: 1,
                    score: 400.0,
                },
                Entry {
                    rank: 2,
                    name: "fennekal".to_string(),
                    user_id: 2,
                    run_id: 2,
                    score: 399.0,
                },
            ],
        };

        let new = Leaderboard {
            timestamp: Duration::from_secs(600),
            entries: vec![
                Entry {
                    rank: 1,
                    name: "fennekal".to_string(),
                    user_id: 2,
                    run_id: 3,
                    score: 410.0,
                },
                Entry {
                    rank: 2,
                    name: "possm".to_string(),
                    user_id: 1,
                    run_id: 1,
                    score: 400.0,
                },
            ],
        };

        let pbs = old.pbs(&new);

        assert_eq!(pbs[0].old.unwrap().name, old.entries[1].name);
        assert_eq!(pbs[0].new.name, new.entries[0].name);
    }
}
