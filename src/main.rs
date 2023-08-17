use hdget::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let hook = hook::Hook::new();

    let mut old = match lb::Leaderboard::from_cache().await {
        Ok(old) => old,
        Err(e) => {
            println!("error reading cache: {}", e);
            let new = lb::Leaderboard::from_site().await?.expect("something went wrong while fetching an intial leaderboard");
            new.cache().await?;
            new
        }
    };

    loop {
        let Some(new) = lb::Leaderboard::from_site().await? else { continue };

        let pbs = old.pbs(&new);

        for pb in &pbs {
            hook.send(&format!("{pb}")).await?;
        }

        if pbs.is_empty() {
            println!("nothing to do");
        } else {
            new.cache().await?;
            old = new;
        }

        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
    }
}
