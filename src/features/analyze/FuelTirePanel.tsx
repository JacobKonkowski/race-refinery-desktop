import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { LapSummary } from "../../shared/types";

interface Props {
  laps: LapSummary[];
}

/** Fuel use, tire temps, and pressures from lap summaries. */
export function FuelTirePanel({ laps }: Props) {
  const fuelData = laps
    .filter((l) => l.fuelUsed != null)
    .map((l) => ({ lapNumber: l.lapNumber, fuelUsed: +(l.fuelUsed!).toFixed(3) }));

  const tireData = laps
    .filter((l) => l.lfTemp != null || l.rfTemp != null || l.lrTemp != null || l.rrTemp != null)
    .map((l) => ({
      lapNumber: l.lapNumber,
      lfTemp: l.lfTemp != null ? Math.round(l.lfTemp) : null,
      rfTemp: l.rfTemp != null ? Math.round(l.rfTemp) : null,
      lrTemp: l.lrTemp != null ? Math.round(l.lrTemp) : null,
      rrTemp: l.rrTemp != null ? Math.round(l.rrTemp) : null,
    }));

  const pressureData = laps
    .filter(
      (l) =>
        l.lfPressure != null ||
        l.rfPressure != null ||
        l.lrPressure != null ||
        l.rrPressure != null,
    )
    .map((l) => ({
      lapNumber: l.lapNumber,
      lfPressure: l.lfPressure != null ? +l.lfPressure.toFixed(1) : null,
      rfPressure: l.rfPressure != null ? +l.rfPressure.toFixed(1) : null,
      lrPressure: l.lrPressure != null ? +l.lrPressure.toFixed(1) : null,
      rrPressure: l.rrPressure != null ? +l.rrPressure.toFixed(1) : null,
    }));

  return (
    <div className="fuel-tire-grid">
      <div className="panel">
        <div className="panel-header">
          <h2>Fuel</h2>
        </div>
        <div className="panel-body">
          {fuelData.length === 0 ? (
            <p className="muted">No fuel data for this session.</p>
          ) : (
            <ResponsiveContainer width="100%" height={160}>
              <BarChart data={fuelData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#262d3a" />
                <XAxis dataKey="lapNumber" stroke="#8b95a5" tick={{ fontSize: 11 }} />
                <YAxis stroke="#8b95a5" tick={{ fontSize: 11 }} width={40} />
                <Tooltip
                  contentStyle={{
                    background: "#12161f",
                    border: "1px solid #262d3a",
                    borderRadius: 6,
                    fontSize: 12,
                  }}
                />
                <Bar dataKey="fuelUsed" name="Fuel used (L)" fill="#3fb950" />
              </BarChart>
            </ResponsiveContainer>
          )}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <h2>Tire temps</h2>
        </div>
        <div className="panel-body">
          {tireData.length === 0 ? (
            <p className="muted">No tire temperature data for this session.</p>
          ) : (
            <ResponsiveContainer width="100%" height={160}>
              <LineChart data={tireData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#262d3a" />
                <XAxis dataKey="lapNumber" stroke="#8b95a5" tick={{ fontSize: 11 }} />
                <YAxis stroke="#8b95a5" tick={{ fontSize: 11 }} width={40} />
                <Tooltip
                  contentStyle={{
                    background: "#12161f",
                    border: "1px solid #262d3a",
                    borderRadius: 6,
                    fontSize: 12,
                  }}
                />
                <Legend />
                <Line type="monotone" dataKey="lfTemp" name="LF" stroke="#f85149" dot={false} connectNulls />
                <Line type="monotone" dataKey="rfTemp" name="RF" stroke="#4aa3ff" dot={false} connectNulls />
                <Line type="monotone" dataKey="lrTemp" name="LR" stroke="#d29922" dot={false} connectNulls />
                <Line type="monotone" dataKey="rrTemp" name="RR" stroke="#3fb950" dot={false} connectNulls />
              </LineChart>
            </ResponsiveContainer>
          )}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <h2>Tire pressures</h2>
        </div>
        <div className="panel-body">
          {pressureData.length === 0 ? (
            <p className="muted">
              No tire pressure data. Re-import after a session that publishes pressure channels.
            </p>
          ) : (
            <ResponsiveContainer width="100%" height={160}>
              <LineChart data={pressureData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#262d3a" />
                <XAxis dataKey="lapNumber" stroke="#8b95a5" tick={{ fontSize: 11 }} />
                <YAxis stroke="#8b95a5" tick={{ fontSize: 11 }} width={40} />
                <Tooltip
                  contentStyle={{
                    background: "#12161f",
                    border: "1px solid #262d3a",
                    borderRadius: 6,
                    fontSize: 12,
                  }}
                />
                <Legend />
                <Line
                  type="monotone"
                  dataKey="lfPressure"
                  name="LF"
                  stroke="#f85149"
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="rfPressure"
                  name="RF"
                  stroke="#4aa3ff"
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="lrPressure"
                  name="LR"
                  stroke="#d29922"
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="rrPressure"
                  name="RR"
                  stroke="#3fb950"
                  dot={false}
                  connectNulls
                />
              </LineChart>
            </ResponsiveContainer>
          )}
        </div>
      </div>
    </div>
  );
}
