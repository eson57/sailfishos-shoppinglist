use std::path::PathBuf;
use std::sync::Arc;

use cxx_qt_lib::{QString, QVariant};
use gettextrs::{bindtextdomain, textdomain};
use ractor::ActorRuntime;
use sailors::sailfishapp::QmlApp;
use tracing_subscriber::{
    EnvFilter, fmt::format::FmtSpan, layer::SubscriberExt as _, util::SubscriberInitExt as _,
};

use shoppinglist_core::{
    actor::Instrumented,
    mesh::{
        local_state::{LocalStateActor, LocalStateArgs},
        synced_state::{SyncedStateActor, SyncedStateArgs},
    },
};

use crate::actor::{ActorRegistry, Actors};
use crate::identity::{APP_NAME, DBUS_NAME, dbus_interface, dbus_path};

use self::actor::wire_core_actors;

mod actor;
#[macro_use]
mod list_model;
mod gui;
mod identity;
mod logging;

fn launch_url() -> Option<String> {
    std::env::args().skip(1).find_map(|arg| {
        let arg = arg.trim();
        if arg.starts_with("shoppinglist:") {
            Some(arg.to_string())
        } else {
            None
        }
    })
}

// Used to keep at most one running instance alive. This is used both when
// clicking the app icon and also when opening a link.
//
// When a link is clicked, a new process is always started (I think?), so we
// check to see if the other instance is already listening on DBus and if so,
// send the share URL.
async fn forward_to_running_instance(url: Option<&str>) -> bool {
    let bus = match zbus::Connection::session().await {
        Ok(bus) => bus,
        Err(err) => {
            tracing::warn!("Could not connect to the session bus to forward the link: {err}");
            return false;
        }
    };

    let registered = bus
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &(DBUS_NAME,),
        )
        .await;
    let registered = match registered {
        Ok(registered) => registered,
        Err(err) => {
            tracing::warn!("Could not check for a running instance: {err}");
            return false;
        }
    };
    let owned = registered.body().deserialize::<bool>().ok();
    if owned != Some(true) {
        return false;
    }

    if let Err(err) = bus
        .call_method(
            Some(DBUS_NAME),
            dbus_path().as_str(),
            Some(dbus_interface().as_str()),
            "openLink",
            &(url.unwrap_or(""),),
        )
        .await
    {
        tracing::warn!("Could not forward shopping list link: {err}");
        return false;
    }
    true
}

fn main() {
    let filter = EnvFilter::builder().parse_lossy(format!(
        "error,{}=trace,shoppinglist_core=info,{}",
        env!("CARGO_CRATE_NAME"),
        std::env::var("RUST_LOG").unwrap_or_default()
    ));
    tracing_subscriber::registry()
        .with(filter)
        .with(logging::layer())
        .with(
            tracing_subscriber::fmt::layer()
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .pretty(),
        )
        .init();

    init_gettext();

    qt_async::run(|| {
        let launch_url = launch_url();
        let launch_url = launch_url.as_deref();
        let mut app = QmlApp::application(APP_NAME.into());
        if zbus::block_on(forward_to_running_instance(launch_url)) {
            return;
        }
        let launch_url = QString::from(launch_url.unwrap_or(""));

        gui::register();

        let mut registry = create_actor_registry();
        let mut translator = qt_i18n::new_translator();

        app.set_title(&QString::from("Shopping List"));
        app.set_property(&QString::from("LaunchUrl"), &QVariant::from(&launch_url));
        app.set_property(
            &QString::from("DBusName"),
            &QVariant::from(&QString::from(DBUS_NAME)),
        );
        app.set_property(
            &QString::from("DBusPath"),
            &QVariant::from(&QString::from(&dbus_path())),
        );
        app.set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
        let _ = app.install_default_translator();
        app.set_object_property(&QString::from("ActorRegistry"), registry.pin_mut());
        app.set_object_property(&QString::from("Tr"), translator.pin_mut());

        registry.pin_mut().finish_shutdown_on_quit();

        // Bundled third party QML modules (Opal), installed alongside the app's QML.
        app.add_import_path(&QmlApp::path_to(&QString::from("qml/modules")).to_qstring());
        app.set_source(&QmlApp::path_to(&QString::from("qml/root.qml")));
        registry.connect();
        app.show_full_screen();
        app.exec();

        tracing::info!("Shutting down");

        registry.finish_shutdown();

        // // QML objects refer to the registry and translator.
        // drop(app);
        // drop(translator);
        // drop(registry);
    })
    .expect("Running application");
}

fn create_actor_registry() -> cxx::UniquePtr<ActorRegistry> {
    let local_args = LocalStateArgs::load(Some(
        identity::config_dir().expect("Finding config directory"),
    ))
    .expect("Loading local state");
    let (local_state, _) =
        ActorRuntime::spawn_instant(None, Instrumented(LocalStateActor), local_args)
            .expect("Starting local state actor");
    let synced_args = SyncedStateArgs {
        local_db: local_state.clone(),
        data_dir: Some(identity::data_dir().expect("Finding data directory")),
    };
    let (synced_state, _) =
        ActorRuntime::spawn_instant(None, Instrumented(SyncedStateActor), synced_args)
            .expect("Starting synced state actor");
    let actors = Actors::new(local_state, synced_state);

    wire_core_actors(&actors).unwrap();

    let actors = Arc::new(actors);

    ActorRegistry::from_actors(actors)
}

fn init_gettext() {
    let domain = APP_NAME;
    textdomain(domain).expect("Failed to set gettext domain");
    bindtextdomain(domain, PathBuf::from("/usr/share/locale"))
        .expect("Failed to bind gettext domain");
}
