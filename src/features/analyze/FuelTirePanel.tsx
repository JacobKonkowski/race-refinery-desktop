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
import { getThemeColors } from "../../shared/themeColors";

interface Props {
  laps: LapSummary[];
}

/** Fuel use, tire temps, and pressures from lap summaries. */
export function FuelTirePanel({ laps }: Props) {
  const colors = getThemeColors();
  const tooltipStyle = {
    background: colors.tooltipBg,
    border: `1px solid ${colors.tooltipBorder}`,
    borderRadius: 6,
    fontSize: 12,
  };

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
                <CartesianGrid strokeDasharray="3 3" stroke={colors.chartGrid} />
                <XAxis
                  dataKey="lapNumber"
                  stroke={colors.chartTick}
                  tick={{ fontSize: 11 }}
                />
                <YAxis stroke={colors.chartTick} tick={{ fontSize: 11 }} width={40} />
                <Tooltip contentStyle={tooltipStyle} />
                <Bar dataKey="fuelUsed" name="Fuel used (L)" fill={colors.fast} />
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
                <CartesianGrid strokeDasharray="3 3" stroke={colors.chartGrid} />
                <XAxis
                  dataKey="lapNumber"
                  stroke={colors.chartTick}
                  tick={{ fontSize: 11 }}
                />
                <YAxis stroke={colors.chartTick} tick={{ fontSize: 11 }} width={40} />
                <Tooltip contentStyle={tooltipStyle} />
                <Legend />
                <Line
                  type="monotone"
                  dataKey="lfTemp"
                  name="LF"
                  stroke={colors.tireLf}
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="rfTemp"
                  name="RF"
                  stroke={colors.tireRf}
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="lrTemp"
                  name="LR"
                  stroke={colors.tireLr}
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="rrTemp"
                  name="RR"
                  stroke={colors.tireRr}
                  dot={false}
                  connectNulls
                />
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
                <CartesianGrid strokeDasharray="3 3" stroke={colors.chartGrid} />
                <XAxis
                  dataKey="lapNumber"
                  stroke={colors.chartTick}
                  tick={{ fontSize: 11 }}
                />
                <YAxis stroke={colors.chartTick} tick={{ fontSize: 11 }} width={40} />
                <Tooltip contentStyle={tooltipStyle} />
                <Legend />
                <Line
                  type="monotone"
                  dataKey="lfPressure"
                  name="LF"
                  stroke={colors.tireLf}
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="rfPressure"
                  name="RF"
                  stroke={colors.tireRf}
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="lrPressure"
                  name="LR"
                  stroke={colors.tireLr}
                  dot={false}
                  connectNulls
                />
                <Line
                  type="monotone"
                  dataKey="rrPressure"
                  name="RR"
                  stroke={colors.tireRr}
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
