use crate::types::Message;
use crate::metrics::MetricsStore;
use tokio::sync::broadcast;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Event {
    InboundMessage(Message),
    OutboundMessage(Message),
    SystemLog { level: String, message: String },
}

pub struct MessageBus {
    tx: broadcast::Sender<Event>,
    inbound_tx: broadcast::Sender<Message>,
    metrics: Option<Arc<MetricsStore>>,
}

impl MessageBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        // Separate input queue with same capacity (impl "drop oldest" policy)
        let (inbound_tx, _) = broadcast::channel(capacity);
        Self { tx, inbound_tx, metrics: None }
    }

    pub fn with_metrics(mut self, metrics: Arc<MetricsStore>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Subscribe to general events (logs, outbound messages, etc.)
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Subscribe to inbound messages (agent commands)
    pub fn subscribe_inbound(&self) -> broadcast::Receiver<Message> {
        self.inbound_tx.subscribe()
    }

    pub fn publish(&self, event: Event) -> Result<usize, broadcast::error::SendError<Event>> {
        if let Event::InboundMessage(ref msg) = event {
            // Also send to dedicated inbound queue
            let _ = self.inbound_tx.send(msg.clone());
            if let Some(metrics) = &self.metrics {
                metrics.inc_messages_in();
            }
        } else if let Event::OutboundMessage(_) = event {
            if let Some(metrics) = &self.metrics {
                metrics.inc_messages_out();
            }
        }
        self.tx.send(event)
    }
}
