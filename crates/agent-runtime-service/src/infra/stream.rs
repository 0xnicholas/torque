use crate::agent::stream::StreamEvent;
use axum::response::sse::Event;

pub fn event_to_sse(event: StreamEvent) -> Result<Event, std::convert::Infallible> {
    Ok(Event::default().data(event.to_sse()))
}
