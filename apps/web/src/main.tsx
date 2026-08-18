import { QueryClientProvider } from "@tanstack/react-query"
import { RouterProvider } from "@tanstack/react-router"
import { StrictMode, useEffect } from "react"
import { createRoot } from "react-dom/client"

import { router } from "./app/router"
import { queryClient } from "./shared/lib/query-client"
import { useThemeStore } from "./shared/stores/theme"
import "./styles/index.css"

function ThemeBoot({ children }: { children: React.ReactNode }) {
  const init = useThemeStore((s) => s.init)
  useEffect(() => init(), [init])
  return children
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeBoot>
        <RouterProvider router={router} />
      </ThemeBoot>
    </QueryClientProvider>
  </StrictMode>,
)
