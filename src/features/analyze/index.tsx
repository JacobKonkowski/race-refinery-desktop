import type { Feature } from "../registry";
import { AnalyzePage } from "./AnalyzePage";
import { navigateToFeature } from "../../shared/navigation";

function AnalyzeHeaderActions() {
  return (
    <button className="btn" onClick={() => navigateToFeature("library")}>
      Change in Lap Library
    </button>
  );
}

export const analyzeFeature: Feature = {
  id: "analyze",
  label: "Analyze",
  path: "/analyze",
  element: <AnalyzePage />,
  HeaderActions: AnalyzeHeaderActions,
};
