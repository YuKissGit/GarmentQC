import ReactDOM from "react-dom/client";
import App from "./App";
import AppErrorBoundary from "./AppErrorBoundary";
import "./styles.css";
import "./report.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <AppErrorBoundary><App /></AppErrorBoundary>
);
