import type { Feature } from "../registry";
import { VoiceStudioPage } from "./VoiceStudioPage";

export const voiceStudioFeature: Feature = {
  id: "voice-studio",
  label: "Voice Studio",
  path: "/voice-studio",
  element: <VoiceStudioPage />,
};
