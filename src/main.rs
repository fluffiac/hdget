use hdget::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let hook = hook::Hook::new();

    // Get cache on startup
    let mut old = match lb::Leaderboard::from_cache().await {
        // we got the cache smoothly
        Ok(old) => old,
        // we couldn't read the cache for some reason :(
        Err(e) => {
            println!("error reading cache: {}", e);
            let new = lb::Leaderboard::from_site()
                .await?
                .expect("something went wrong while fetching an intial leaderboard");
            new.cache().await?;
            new
        }
    };

    loop {
        // wait 10 mins
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;

        // create a new Leaderboard object by scraping the site
        // if this fails, 
        let Some(new) = lb::Leaderboard::from_site().await? else { continue };

        // get all pbs (difference of old to new)
        let pbs = old.pbs(&new);

        if pbs.is_empty() {
            println!("nothing to do");
        } else {
            // send each pb to the webhook            
            for pb in &pbs {
                hook.send(&pb.to_string()).await?;
            }

            // cache the new leaderboard
            new.cache().await?;
            old = new;
        }
    }
}
