import { useState } from "react";
import { testProviderConnection } from "../../lib/tauri";
import { formatUnixDate } from "../../lib/utils";
import { useProviderStore } from "../../store/providerStore";
import type {
  ConnectionTestResult,
  Provider,
  ProviderInput,
  ProviderType,
} from "../../types";

interface ProviderFormProps {
  initial?: Provider | null;
  onSaved: (provider: Provider) => void;
  onCancel?: () => void;
}

type M3uSource = "url" | "file";

const inputClass =
  "w-full rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 placeholder-zinc-500 outline-none focus:border-zinc-400";

const labelClass = "mb-1 block text-xs font-medium text-zinc-400";

export default function ProviderForm({
  initial,
  onSaved,
  onCancel,
}: ProviderFormProps) {
  const save = useProviderStore((s) => s.save);

  const [name, setName] = useState(initial?.name ?? "");
  const [type, setType] = useState<ProviderType>(initial?.type ?? "xtream");
  const [serverUrl, setServerUrl] = useState(initial?.serverUrl ?? "");
  const [username, setUsername] = useState(initial?.username ?? "");
  const [password, setPassword] = useState("");
  const [m3uSource, setM3uSource] = useState<M3uSource>(
    initial?.localFilePath ? "file" : "url",
  );
  const [playlistUrl, setPlaylistUrl] = useState(initial?.playlistUrl ?? "");
  const [localFilePath, setLocalFilePath] = useState(
    initial?.localFilePath ?? "",
  );

  const [error, setError] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ConnectionTestResult | null>(
    null,
  );
  const [saving, setSaving] = useState(false);

  const isEdit = Boolean(initial);

  function validate(): string | null {
    if (!name.trim()) return "Provider name is required.";
    if (type === "xtream") {
      if (!serverUrl.trim()) return "Server URL is required.";
      if (!username.trim()) return "Username is required.";
      if (!isEdit && !password) return "Password is required.";
    } else {
      if (m3uSource === "url" && !playlistUrl.trim())
        return "Playlist URL is required.";
      if (m3uSource === "file" && !localFilePath.trim())
        return "Playlist file path is required.";
    }
    return null;
  }

  function buildInput(): ProviderInput {
    const input: ProviderInput = { name: name.trim(), type };
    if (initial) input.id = initial.id;
    if (type === "xtream") {
      input.serverUrl = serverUrl.trim();
      input.username = username.trim();
      if (password) input.password = password;
    } else if (m3uSource === "url") {
      input.playlistUrl = playlistUrl.trim();
    } else {
      input.localFilePath = localFilePath.trim();
    }
    return input;
  }

  async function handleTest() {
    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }
    setError(null);
    setTesting(true);
    setTestResult(null);
    try {
      setTestResult(await testProviderConnection(buildInput()));
    } catch (e) {
      setTestResult({ success: false, message: String(e), accountInfo: null });
    } finally {
      setTesting(false);
    }
  }

  async function handleSave() {
    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }
    setError(null);
    setSaving(true);
    try {
      const saved = await save(buildInput());
      onSaved(saved);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  const typeTab = (value: ProviderType, label: string) => (
    <button
      type="button"
      onClick={() => {
        setType(value);
        setTestResult(null);
      }}
      className={`rounded-md px-4 py-1.5 text-sm font-medium transition-colors ${
        type === value
          ? "bg-zinc-200 text-zinc-900"
          : "text-zinc-400 hover:text-zinc-100"
      }`}
    >
      {label}
    </button>
  );

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        void handleSave();
      }}
    >
      <div>
        <label className={labelClass} htmlFor="provider-name">
          Provider Name
        </label>
        <input
          id="provider-name"
          className={inputClass}
          placeholder="My IPTV Provider"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </div>

      <div>
        <span className={labelClass}>Provider Type</span>
        <div className="inline-flex gap-1 rounded-lg border border-zinc-800 bg-zinc-900 p-1">
          {typeTab("xtream", "Xtream Codes")}
          {typeTab("m3u", "M3U Playlist")}
        </div>
      </div>

      {type === "xtream" ? (
        <>
          <div>
            <label className={labelClass} htmlFor="server-url">
              Server URL
            </label>
            <input
              id="server-url"
              className={inputClass}
              placeholder="http://example.com:8080"
              value={serverUrl}
              onChange={(e) => setServerUrl(e.target.value)}
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className={labelClass} htmlFor="username">
                Username
              </label>
              <input
                id="username"
                className={inputClass}
                autoComplete="off"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
              />
            </div>
            <div>
              <label className={labelClass} htmlFor="password">
                Password
              </label>
              <input
                id="password"
                type="password"
                className={inputClass}
                autoComplete="new-password"
                placeholder={isEdit ? "Leave blank to keep current" : ""}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </div>
          </div>
        </>
      ) : (
        <>
          <div>
            <span className={labelClass}>Playlist Source</span>
            <div className="flex gap-4 text-sm text-zinc-300">
              <label className="flex items-center gap-2">
                <input
                  type="radio"
                  name="m3u-source"
                  checked={m3uSource === "url"}
                  onChange={() => setM3uSource("url")}
                />
                URL
              </label>
              <label className="flex items-center gap-2">
                <input
                  type="radio"
                  name="m3u-source"
                  checked={m3uSource === "file"}
                  onChange={() => setM3uSource("file")}
                />
                Local file
              </label>
            </div>
          </div>
          {m3uSource === "url" ? (
            <div>
              <label className={labelClass} htmlFor="playlist-url">
                Playlist URL
              </label>
              <input
                id="playlist-url"
                className={inputClass}
                placeholder="http://example.com/playlist.m3u"
                value={playlistUrl}
                onChange={(e) => setPlaylistUrl(e.target.value)}
              />
            </div>
          ) : (
            <div>
              <label className={labelClass} htmlFor="file-path">
                Playlist File Path
              </label>
              <input
                id="file-path"
                className={inputClass}
                placeholder="C:\\playlists\\channels.m3u"
                value={localFilePath}
                onChange={(e) => setLocalFilePath(e.target.value)}
              />
            </div>
          )}
        </>
      )}

      {error && (
        <p className="rounded-md border border-red-900 bg-red-950/50 px-3 py-2 text-sm text-red-300">
          {error}
        </p>
      )}

      {testResult && (
        <div
          className={`rounded-md border px-3 py-2 text-sm ${
            testResult.success
              ? "border-emerald-900 bg-emerald-950/50 text-emerald-300"
              : "border-red-900 bg-red-950/50 text-red-300"
          }`}
        >
          <p>{testResult.message}</p>
          {testResult.accountInfo && (
            <ul className="mt-1 text-xs text-zinc-400">
              {testResult.accountInfo.status && (
                <li>Status: {testResult.accountInfo.status}</li>
              )}
              {testResult.accountInfo.expDate !== null && (
                <li>Expires: {formatUnixDate(testResult.accountInfo.expDate)}</li>
              )}
              {testResult.accountInfo.maxConnections !== null && (
                <li>
                  Connections: {testResult.accountInfo.activeConnections ?? 0} /{" "}
                  {testResult.accountInfo.maxConnections} active
                </li>
              )}
            </ul>
          )}
        </div>
      )}

      <div className="flex items-center gap-2 pt-1">
        <button
          type="submit"
          disabled={saving}
          className="rounded-md bg-zinc-100 px-4 py-2 text-sm font-semibold text-zinc-900 transition-colors hover:bg-white disabled:opacity-50"
        >
          {saving ? "Saving…" : isEdit ? "Save Changes" : "Add Provider"}
        </button>
        <button
          type="button"
          onClick={() => void handleTest()}
          disabled={testing}
          className="rounded-md border border-zinc-700 px-4 py-2 text-sm font-medium text-zinc-200 transition-colors hover:bg-zinc-900 disabled:opacity-50"
        >
          {testing ? "Testing…" : "Test Connection"}
        </button>
        {onCancel && (
          <button
            type="button"
            onClick={onCancel}
            className="ml-auto rounded-md px-4 py-2 text-sm text-zinc-400 hover:text-zinc-100"
          >
            Cancel
          </button>
        )}
      </div>
    </form>
  );
}
