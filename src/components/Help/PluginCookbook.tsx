import { useTranslation } from 'react-i18next';
import { BookOpen, Code, ExternalLink, Lightbulb } from 'lucide-react';

export default function PluginCookbook() {
  const { t } = useTranslation();

  return (
    <div className="max-w-3xl mx-auto p-6 space-y-6">
      <div className="flex items-center gap-3 mb-4">
        <BookOpen size={24} className="text-accent-primary" />
        <h1 className="text-xl font-bold text-text-primary">
          {t('plugin.cookbook')}
        </h1>
      </div>

      {/* Getting Started */}
      <section className="space-y-3">
        <h2 className="flex items-center gap-2 text-lg font-semibold text-text-primary">
          <Lightbulb size={16} />
          {t('plugin.gettingStarted')}
        </h2>
        <div className="bg-surface-secondary rounded-lg p-4 space-y-2 text-sm text-text-secondary">
          <p>
            CrossTerm plugins are WASM modules that extend the terminal with custom functionality.
            Plugins can hook into session lifecycle events, add sidebar panels, contribute context
            menu items, and store per-plugin key-value data.
          </p>
          <p>
            To create a plugin, you need a <code className="text-accent-primary">manifest.json</code> file
            and a compiled <code className="text-accent-primary">.wasm</code> entry point.
          </p>
        </div>
      </section>

      {/* Manifest Example */}
      <section className="space-y-3">
        <h2 className="flex items-center gap-2 text-lg font-semibold text-text-primary">
          <Code size={16} />
          manifest.json
        </h2>
        <pre className="bg-surface-sunken rounded-lg p-4 text-xs font-mono text-text-primary overflow-x-auto">
{`{
  "id": "my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "author": "Your Name",
  "description": "A sample CrossTerm plugin",
  "permissions": ["terminal", "notifications"],
  "entry_point": "plugin.wasm",
  "api_version": "1.0"
}`}
        </pre>
      </section>

      {/* Lifecycle Hooks */}
      <section className="space-y-3">
        <h2 className="flex items-center gap-2 text-lg font-semibold text-text-primary">
          <Code size={16} />
          {t('plugin.hooks')}
        </h2>
        <div className="bg-surface-secondary rounded-lg p-4 text-sm text-text-secondary space-y-2">
          <p>Plugins can register for the following lifecycle hooks:</p>
          <ul className="list-disc list-inside space-y-1">
            <li><code className="text-accent-primary">on_connect</code> — Fired when a session connects</li>
            <li><code className="text-accent-primary">on_disconnect</code> — Fired when a session disconnects</li>
            <li><code className="text-accent-primary">on_output_line</code> — Fired for each output line</li>
            <li><code className="text-accent-primary">on_command</code> — Fired when a command is entered</li>
            <li><code className="text-accent-primary">on_session_start</code> — Fired when a session starts</li>
            <li><code className="text-accent-primary">on_session_end</code> — Fired when a session ends</li>
          </ul>
        </div>
      </section>

      {/* KV Store */}
      <section className="space-y-3">
        <h2 className="flex items-center gap-2 text-lg font-semibold text-text-primary">
          <Code size={16} />
          {t('plugin.kvStore')}
        </h2>
        <pre className="bg-surface-sunken rounded-lg p-4 text-xs font-mono text-text-primary overflow-x-auto">
{`// Store data
await invoke('plugin_kv_set', {
  pluginId: 'my-plugin',
  key: 'last_run',
  value: new Date().toISOString()
});

// Retrieve data
const value = await invoke('plugin_kv_get', {
  pluginId: 'my-plugin',
  key: 'last_run'
});`}
        </pre>
      </section>

      {/* API Reference Links */}
      <section className="space-y-3">
        <h2 className="flex items-center gap-2 text-lg font-semibold text-text-primary">
          <ExternalLink size={16} />
          API Reference
        </h2>
        <div className="bg-surface-secondary rounded-lg p-4 text-sm text-text-secondary space-y-1">
          <p>For full API documentation, see the Plugin API Guide in the help section.</p>
        </div>
      </section>
    </div>
  );
}
