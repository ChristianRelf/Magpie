import {
  createContext,
  useCallback,
  useContext,
  useState,
  type ReactNode,
} from "react";

export type Route =
  | "overview"
  | "providers"
  | "models"
  | "activity"
  | "analytics"
  | "routing"
  | "integrations"
  | "settings";

export const ROUTES: Route[] = [
  "overview",
  "providers",
  "models",
  "activity",
  "analytics",
  "routing",
  "integrations",
  "settings",
];

interface NavState {
  route: Route;
  /** Optional parameter, e.g. an execution id for the activity inspector. */
  param: string | null;
  navigate: (route: Route, param?: string | null) => void;
}

const NavContext = createContext<NavState | null>(null);

export function NavProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<{ route: Route; param: string | null }>(
    () => {
      const saved = localStorage.getItem("magpie.route") as Route | null;
      return {
        route: saved && ROUTES.includes(saved) ? saved : "overview",
        param: null,
      };
    },
  );
  const navigate = useCallback((route: Route, param: string | null = null) => {
    localStorage.setItem("magpie.route", route);
    setState({ route, param });
  }, []);
  return (
    <NavContext.Provider value={{ ...state, navigate }}>
      {children}
    </NavContext.Provider>
  );
}

export function useNav(): NavState {
  const ctx = useContext(NavContext);
  if (!ctx) throw new Error("useNav outside provider");
  return ctx;
}
