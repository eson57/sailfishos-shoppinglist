use std::str::FromStr;

use iroh_docs::DocTicket;
use iroh_tickets::Ticket;
use n0_error::{Result, StdResultExt as _, anyerr};
use url::Url;

pub const SCHEME: &str = "shoppinglist";

/// Build a share link for a shopping list.
/// A share link is simply the iroh DocTicket base64'd into the path: `shoppinglist://ticket/<payload>`.
pub fn format_sharing_link(ticket: &DocTicket) -> String {
    let payload = ticket.encode_string();
    format!("{SCHEME}://ticket/{payload}")
}

pub fn parse_sharing_link(input: &str) -> Result<DocTicket> {
    let url = Url::parse(input.trim()).anyerr()?;
    let segments: Vec<_> = url
        .path_segments()
        .anyerr()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let &[payload] = segments.as_slice() else {
        return Err(anyerr!("Share URL lacked payload"));
    };

    DocTicket::from_str(payload).anyerr()
}
