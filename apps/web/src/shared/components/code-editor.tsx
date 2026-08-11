import Editor, { loader } from "@monaco-editor/react"
import * as monaco from "monaco-editor"
import editorWorker from "@monaco/esm/vs/editor/editor.worker.js?worker"
import jsonWorker from "@monaco/esm/vs/language/json/json.worker.js?worker"

type MonacoWorkerEnvironment = {
  getWorker: (_moduleId: string, label: string) => Worker
}

loader.config({ monaco })

;(globalThis as typeof globalThis & { MonacoEnvironment: MonacoWorkerEnvironment })
  .MonacoEnvironment = {
  getWorker: (_moduleId, label) => {
    if (label === "json") return new jsonWorker()
    return new editorWorker()
  },
}

export default Editor
