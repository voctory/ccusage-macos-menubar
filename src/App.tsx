import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-opener";
import "./App.css";

interface ModelBreakdown {
  modelName: string;
  inputTokens: number;
  outputTokens: number;
  cacheCreationTokens: number;
  cacheReadTokens: number;
  cost: number;
}

interface UsageData {
  today_data: ModelBreakdown[] | null;
  five_hr_data: ModelBreakdown[] | null;
  one_hr_data: ModelBreakdown[] | null;
  week_data: ModelBreakdown[] | null;
}

function formatModelName(modelName: string): string {
  switch (modelName) {
    case "claude-opus-4-20250514":
      return "Opus 4";
    case "claude-sonnet-4-20250514":
      return "Sonnet 4";
    case "claude-3-5-sonnet-20241022":
      return "Sonnet 3.5";
    case "claude-3-haiku-20240307":
      return "Haiku";
    default:
      if (modelName.includes("opus")) return "Opus";
      if (modelName.includes("sonnet")) return "Sonnet";
      if (modelName.includes("haiku")) return "Haiku";
      return modelName;
  }
}

function UsageSection({ title, data }: { title: string; data: ModelBreakdown[] | null }) {
  return (
    <div className="usage-section">
      <div className="section-title">{title}</div>
      {data ? (
        data.map((breakdown) => (
          <div key={breakdown.modelName} className="usage-item">
            <span className="model-name">{formatModelName(breakdown.modelName)}:</span>
            <span className="cost">${breakdown.cost.toFixed(2)}</span>
            <span className="tokens">
              (In: {(breakdown.inputTokens / 1000).toFixed(1)}K, Out: {(breakdown.outputTokens / 1000).toFixed(1)}K)
            </span>
          </div>
        ))
      ) : (
        <div className="no-data">No data available</div>
      )}
    </div>
  );
}

function App() {
  const [usageData, setUsageData] = useState<UsageData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [autoStartEnabled, setAutoStartEnabled] = useState(false);

  const fetchData = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await invoke<UsageData>("fetch_all_usage_data");
      setUsageData(data);
    } catch (err) {
      setError(err as string);
    } finally {
      setLoading(false);
    }
  };

  const handleRefresh = async () => {
    await fetchData();
  };

  const handleCCUsageClick = () => {
    open("https://github.com/ryoppippi/ccusage");
  };

  const handleToggleAutoStart = async () => {
    try {
      const newState = await invoke<boolean>("toggle_autostart");
      setAutoStartEnabled(newState);
    } catch (err) {
      console.error("Failed to toggle autostart:", err);
    }
  };

  useEffect(() => {
    fetchData();
    
    // Check autostart status
    invoke<boolean>("is_autostart_enabled")
      .then(setAutoStartEnabled)
      .catch(console.error);
  }, []);

  return (
    <div className="popup-container">
      <div className="caret"></div>
      
      <div className="popup-content">
        <div className="header" onClick={handleCCUsageClick}>
          CCUsage
        </div>

        {loading ? (
          <div className="loading">
            <div className="spinner"></div>
            Loading usage data...
          </div>
        ) : error ? (
          <div className="error">
            Failed to load data: {error}
            <button onClick={handleRefresh}>Retry</button>
          </div>
        ) : (
          <>
            <UsageSection title="1 Hr" data={usageData?.one_hr_data || null} />
            <UsageSection title="5 Hr" data={usageData?.five_hr_data || null} />
            <UsageSection title="Today" data={usageData?.today_data || null} />
            <UsageSection title="Week" data={usageData?.week_data || null} />
          </>
        )}

        <div className="controls">
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={autoStartEnabled}
              onChange={handleToggleAutoStart}
            />
            Launch on startup
          </label>
          
          <button className="refresh-btn" onClick={handleRefresh} disabled={loading}>
            Refresh
          </button>
        </div>
      </div>
    </div>
  );
}

export default App;