// LLM generated debug console

use n0_error::{Result, StackResultExt as _, StdResultExt as _, anyerr};
use ractor::{Actor, ActorRef, call};
use shoppinglist_core::{
    actor::{FlattenCall as _, Instrumented},
    link::parse_sharing_link,
    mesh::{
        local_state::{LocalStateActor, LocalStateArgs},
        shoppinglist::ShoppingListMsg,
        synced_state::{SyncedStateActor, SyncedStateArgs, SyncedStateMsg},
    },
    models::{ShoppingListId, ShoppingListItem, ShoppingListItemId, ShoppingListWithId},
};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

const HELP: &str = "\
commands:
  lists                      show the shopping lists
  new <name>                 create a shopping list
  import <link>              join a shared shopping list
  show <list>                show a shopping list's items
  add <list> <item>          add an item
  rename <list> <n> <name>   rename item number n
  share <list>               print a share link
  remove <list>              forget (and delete) a shopping list
  report                     show the debug report
  help                       show this
  quit                       stop
<list> is a number from `lists`, or the start of a shopping list's id.";

struct Console {
    synced_state: ActorRef<SyncedStateMsg>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("warn,shoppinglist_core=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let (local_state, _) = Actor::spawn(
        None,
        Instrumented(LocalStateActor),
        LocalStateArgs::load(None)?,
    )
    .await
    .map_err(|err| anyerr!("Could not start local state: {err}"))?;
    let (synced_state, _) = Actor::spawn(
        None,
        Instrumented(SyncedStateActor),
        SyncedStateArgs {
            local_db: local_state.clone(),
            data_dir: None,
        },
    )
    .await
    .map_err(|err| anyerr!("Could not start synced state: {err}"))?;

    let console = Console { synced_state };

    println!("{HELP}");
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    loop {
        let line = tokio::select! {
            line = lines.next_line() => line.map_err(|err| anyerr!("Could not read input: {err}"))?,
            _ = tokio::signal::ctrl_c() => None,
        };
        let Some(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, "quit" | "exit") {
            break;
        }
        if let Err(err) = console.run(line).await {
            println!("error: {err:?}");
        }
    }

    stop(&console.synced_state).await;
    stop(&local_state).await;
    Ok(())
}

async fn stop<M: ractor::Message>(actor: &ActorRef<M>) {
    if let Err(err) = actor.get_cell().stop_and_wait(None, None).await {
        eprintln!("An actor did not stop cleanly: {err:?}");
    }
}

impl Console {
    async fn run(&self, line: &str) -> Result<()> {
        let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match command {
            "help" => println!("{HELP}"),
            "lists" => self.lists().await?,
            "new" => self.new_list(rest).await?,
            "import" => self.import(rest).await?,
            "show" => self.show(rest).await?,
            "add" => self.add(rest).await?,
            "rename" => self.rename(rest).await?,
            "share" => self.share(rest).await?,
            "remove" => self.remove(rest).await?,
            "report" => self.report().await?,
            _ => println!("unknown command {command:?}, try `help`"),
        }
        Ok(())
    }

    async fn all(&self) -> Result<Vec<ShoppingListWithId>> {
        call!(self.synced_state, SyncedStateMsg::ListShoppingLists).flatten_call()
    }

    /// Finds a shopping list from a number in `lists`, or the start of its id.
    async fn find(&self, selector: &str) -> Result<ShoppingListWithId> {
        let all = self.all().await?;
        if let Ok(number) = selector.parse::<usize>() {
            return number
                .checked_sub(1)
                .and_then(|index| all.get(index))
                .cloned()
                .context("No shopping list has that number");
        }
        let mut matching = all
            .into_iter()
            .filter(|list| !selector.is_empty() && list.id.to_string().starts_with(selector));
        match (matching.next(), matching.next()) {
            (Some(list), None) => Ok(list),
            (None, _) => Err(anyerr!("No shopping list matches {selector:?}")),
            _ => Err(anyerr!("More than one shopping list matches {selector:?}")),
        }
    }

    async fn actor(&self, id: ShoppingListId) -> Result<ActorRef<ShoppingListMsg>> {
        call!(self.synced_state, |reply| {
            SyncedStateMsg::OpenShoppingList {
                id,
                ticket: None,
                reply,
            }
        })
        .flatten_call()
    }

    async fn lists(&self) -> Result<()> {
        let all = self.all().await?;
        if all.is_empty() {
            println!("no shopping lists");
        }
        for (index, list) in all.iter().enumerate() {
            println!(
                "{}. {:?} ({} items) {}",
                index + 1,
                list.inner.name,
                list.inner.items.len(),
                list.id
            );
        }
        Ok(())
    }

    async fn new_list(&self, name: &str) -> Result<()> {
        let actor = call!(
            self.synced_state,
            SyncedStateMsg::CreateShoppingList,
            name.to_owned()
        )
        .flatten_call()?;
        println!("created {}", call!(actor, ShoppingListMsg::GetId).anyerr()?);
        Ok(())
    }

    async fn import(&self, link: &str) -> Result<()> {
        let ticket = parse_sharing_link(link)?;
        let id = ShoppingListId {
            id: ticket.capability.id(),
        };
        call!(self.synced_state, |reply| {
            SyncedStateMsg::OpenShoppingList {
                id,
                ticket: Some(ticket),
                reply,
            }
        })
        .flatten_call()?;
        println!("joined {id}");
        Ok(())
    }

    async fn show(&self, selector: &str) -> Result<()> {
        let list = self.find(selector).await?;
        println!("{:?} ({})", list.inner.name, list.id);
        for (number, item) in list.inner.items.values().enumerate() {
            let mark = if item.completed_at.is_some() {
                'x'
            } else {
                ' '
            };
            println!("  {}. [{mark}] {}", number + 1, item.name);
        }
        Ok(())
    }

    async fn add(&self, args: &str) -> Result<()> {
        let (selector, name) = args.split_once(' ').context("Usage: add <list> <item>")?;
        let list = self.find(selector).await?;
        let item = ShoppingListItem {
            id: ShoppingListItemId::new(Uuid::new_v4()),
            name: name.trim().to_owned(),
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        call!(self.actor(list.id).await?, ShoppingListMsg::AddItem, item).flatten_call()?;
        Ok(())
    }

    async fn rename(&self, args: &str) -> Result<()> {
        let mut parts = args.splitn(3, ' ');
        let (Some(selector), Some(number), Some(name)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(anyerr!("Usage: rename <list> <n> <name>"));
        };
        let list = self.find(selector).await?;
        let number: usize = number.parse().std_context("<n> must be a number")?;
        let id = number
            .checked_sub(1)
            .and_then(|index| list.inner.items.keys().nth(index))
            .copied()
            .context("The list has no item with that number")?;
        let name = name.trim().to_owned();
        call!(self.actor(list.id).await?, |reply| {
            ShoppingListMsg::UpdateItem { id, name, reply }
        })
        .flatten_call()?;
        Ok(())
    }

    async fn share(&self, selector: &str) -> Result<()> {
        let list = self.find(selector).await?;
        let link = call!(self.actor(list.id).await?, ShoppingListMsg::Share).flatten_call()?;
        println!("{link}");
        Ok(())
    }

    async fn remove(&self, selector: &str) -> Result<()> {
        let list = self.find(selector).await?;
        call!(
            self.synced_state,
            SyncedStateMsg::RemoveShoppingList,
            list.id
        )
        .flatten_call()?;
        println!("removed {}", list.id);
        Ok(())
    }

    async fn report(&self) -> Result<()> {
        let report = call!(self.synced_state, SyncedStateMsg::GetDebugReport).flatten_call()?;
        println!("{report:#?}");
        Ok(())
    }
}
