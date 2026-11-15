import { AppShell } from "./shell/AppShell";
import { features } from "./features/registry";
import { ThemeProvider } from "./shared/ThemeProvider";

export default function App() {
  return (
    <ThemeProvider>
      <AppShell features={features} />
    </ThemeProvider>
  );
}
