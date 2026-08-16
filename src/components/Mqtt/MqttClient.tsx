import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, AlertTriangle, Radio, Send, X } from "lucide-react";
import type { MqttConfig, MqttMessage, MqttQos } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface MqttClientProps {
  readonly sessionId: string;
  readonly config: MqttConfig;
}

const QOS_OPTIONS: { value: MqttQos; label: string }[] = [
  { value: "AtMostOnce", label: "0 — At most once" },
  { value: "AtLeastOnce", label: "1 — At least once" },
  { value: "ExactlyOnce", label: "2 — Exactly once" },
];

const MAX_LOG = 500;

export default function MqttClient({ sessionId, config }: MqttClientProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const connectionIdRef = useRef<string | null>(null);
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);

  const [subscribeInput, setSubscribeInput] = useState("#");
  const [subscribedTopics, setSubscribedTopics] = useState<string[]>([]);
  const [messages, setMessages] = useState<MqttMessage[]>([]);
  const logEndRef = useRef<HTMLDivElement>(null);

  const [publishTopic, setPublishTopic] = useState("");
  const [publishPayload, setPublishPayload] = useState("");
  const [publishQos, setPublishQos] = useState<MqttQos>("AtMostOnce");
  const [publishRetain, setPublishRetain] = useState(false);
  const [publishing, setPublishing] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let unlistenMessage: (() => void) | null = null;
    setStatus("connecting");
    setError(null);

    invoke<string>("mqtt_connect", { config })
      .then(async (id) => {
        if (cancelled) {
          invoke("mqtt_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
        // Attach the listener before flipping to "connected" — the broker
        // can start delivering messages as soon as connect resolves, so a
        // separate effect keyed on connectionId left a window where
        // messages could arrive before anything was listening for them.
        const unlisten = await listen<MqttMessage>("mqtt:message", (event) => {
          if (event.payload.session_id !== id) return;
          setMessages((prev) => {
            const next = [...prev, event.payload];
            return next.length > MAX_LOG ? next.slice(next.length - MAX_LOG) : next;
          });
        });
        if (cancelled) {
          unlisten();
          return;
        }
        unlistenMessage = unlisten;
        setConnectionId(id);
        setStatus("connected");
      })
      .catch((e) => {
        if (!cancelled) {
          setStatus("error");
          setError(String(e));
        }
      });

    return () => {
      cancelled = true;
      unlistenMessage?.();
      const id = connectionIdRef.current;
      if (id) invoke("mqtt_disconnect", { id }).catch(() => {});
      connectionIdRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ block: "end" });
  }, [messages]);

  const handleSubscribe = useCallback(async () => {
    const topic = subscribeInput.trim();
    if (!connectionId || !topic || subscribedTopics.includes(topic)) return;
    try {
      await invoke("mqtt_subscribe", { id: connectionId, topic, qos: "AtMostOnce" satisfies MqttQos });
      setSubscribedTopics((prev) => [...prev, topic]);
    } catch (e) {
      toast("error", `Subscribe failed: ${String(e)}`);
    }
  }, [connectionId, subscribeInput, subscribedTopics, toast]);

  const handleUnsubscribe = useCallback(
    async (topic: string) => {
      if (!connectionId) return;
      try {
        await invoke("mqtt_unsubscribe", { id: connectionId, topic });
        setSubscribedTopics((prev) => prev.filter((t) => t !== topic));
      } catch (e) {
        toast("error", `Unsubscribe failed: ${String(e)}`);
      }
    },
    [connectionId, toast]
  );

  const handlePublish = useCallback(async () => {
    if (!connectionId || !publishTopic.trim()) return;
    setPublishing(true);
    try {
      await invoke("mqtt_publish", {
        id: connectionId,
        topic: publishTopic.trim(),
        payload: publishPayload,
        qos: publishQos,
        retain: publishRetain,
      });
      toast("success", `Published to ${publishTopic.trim()}`);
    } catch (e) {
      toast("error", `Publish failed: ${String(e)}`);
    } finally {
      setPublishing(false);
    }
  }, [connectionId, publishTopic, publishPayload, publishQos, publishRetain, toast]);

  if (status === "connecting") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Connecting to {config.host}:{config.port}…</span>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't connect to {config.host}:{config.port}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-3 overflow-hidden p-4">
      <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
        <Radio size={16} className="text-accent-primary" />
        MQTT — {config.host}:{config.port}
      </h2>

      <div className="flex items-center gap-2">
        <input
          value={subscribeInput}
          onChange={(e) => setSubscribeInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSubscribe()}
          placeholder="Topic filter, e.g. sensors/#"
          className="flex-1 rounded-md border border-border-default bg-surface-primary px-3 py-1.5 text-sm text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
        />
        <button
          onClick={handleSubscribe}
          className="rounded-md bg-interactive-default px-3 py-1.5 text-sm font-medium text-text-inverse hover:bg-interactive-hover transition-colors"
        >
          Subscribe
        </button>
      </div>

      {subscribedTopics.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {subscribedTopics.map((topic) => (
            <span
              key={topic}
              className="flex items-center gap-1 rounded bg-surface-elevated px-2 py-1 text-xs text-text-secondary"
            >
              {topic}
              <button onClick={() => handleUnsubscribe(topic)} title="Unsubscribe">
                <X size={11} className="hover:text-status-disconnected" />
              </button>
            </span>
          ))}
        </div>
      )}

      <div className="flex-1 overflow-auto rounded-md border border-border-default bg-surface-sunken p-2 font-mono text-xs">
        {messages.length === 0 ? (
          <div className="p-2 text-text-disabled">No messages yet — subscribe to a topic to see live traffic.</div>
        ) : (
          messages.map((m, i) => (
            <div key={i} className="border-b border-border-subtle py-1 last:border-0">
              <span className="text-accent-primary">{m.topic}</span>
              {m.retain && <span className="ml-1 text-text-disabled">[retained]</span>}
              <span className="ml-2 text-text-secondary break-all">{m.payload}</span>
            </div>
          ))
        )}
        <div ref={logEndRef} />
      </div>

      <div className="flex flex-col gap-2 rounded-md border border-border-default p-2">
        <div className="flex items-center gap-2">
          <input
            value={publishTopic}
            onChange={(e) => setPublishTopic(e.target.value)}
            placeholder="Publish topic"
            className="flex-1 rounded-md border border-border-default bg-surface-primary px-2 py-1 text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          />
          <select
            value={publishQos}
            onChange={(e) => setPublishQos(e.target.value as MqttQos)}
            className="rounded-md border border-border-default bg-surface-primary px-2 py-1 text-xs text-text-primary"
          >
            {QOS_OPTIONS.map((q) => (
              <option key={q.value} value={q.value}>{q.label}</option>
            ))}
          </select>
          <label className="flex items-center gap-1 text-xs text-text-secondary">
            <input type="checkbox" checked={publishRetain} onChange={(e) => setPublishRetain(e.target.checked)} />
            Retain
          </label>
        </div>
        <div className="flex items-center gap-2">
          <input
            value={publishPayload}
            onChange={(e) => setPublishPayload(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handlePublish()}
            placeholder="Payload"
            className="flex-1 rounded-md border border-border-default bg-surface-primary px-2 py-1 text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          />
          <button
            onClick={handlePublish}
            disabled={publishing || !publishTopic.trim()}
            className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors disabled:opacity-50"
          >
            <Send size={12} />
            Publish
          </button>
        </div>
      </div>
    </div>
  );
}
