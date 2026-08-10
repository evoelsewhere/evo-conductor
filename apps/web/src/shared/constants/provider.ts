import anthropicUrl from "@/assets/providers/anthropic.svg?url"
import azureAiUrl from "@/assets/providers/azureai.svg?url"
import bedrockUrl from "@/assets/providers/bedrock.svg?url"
import deepSeekUrl from "@/assets/providers/deepseek.svg?url"
import fptUrl from "@/assets/providers/fpt.svg?url"
import githubCopilotUrl from "@/assets/providers/githubcopilot.svg?url"
import googleUrl from "@/assets/providers/google.svg?url"
import kimiUrl from "@/assets/providers/kimi.svg?url"
import nvidiaUrl from "@/assets/providers/nvidia.svg?url"
import ollamaUrl from "@/assets/providers/ollama.svg?url"
import openAiUrl from "@/assets/providers/openai.svg?url"
import openRouterUrl from "@/assets/providers/openrouter.svg?url"
import xAiUrl from "@/assets/providers/xai.svg?url"
import xiaomiMiMoUrl from "@/assets/providers/xiaomimimo.svg?url"
import zaiUrl from "@/assets/providers/zai.svg?url"

export interface ProviderBrand {
  color: string
  maskUrl?: string
  imageUrl?: string
  label: string
}

export const PROVIDER_BRANDS: Record<string, ProviderBrand> = {
  anthropic: { color: "#d4a574", maskUrl: anthropicUrl, label: "Anthropic" },
  googlegenai: { color: "#4285f4", maskUrl: googleUrl, label: "Google Gemini" },
  vertexai: { color: "#4285f4", maskUrl: googleUrl, label: "Vertex AI" },
  openai: { color: "#10a37f", maskUrl: openAiUrl, label: "OpenAI" },
  codex: { color: "#10a37f", maskUrl: openAiUrl, label: "Codex" },
  openrouter: { color: "#8b5cf6", maskUrl: openRouterUrl, label: "OpenRouter" },
  zai: { color: "#6b7280", maskUrl: zaiUrl, label: "Z.ai" },
  nvidia: { color: "#76b900", maskUrl: nvidiaUrl, label: "NVIDIA" },
  xai: { color: "#9ca3af", maskUrl: xAiUrl, label: "xAI" },
  deepseek: { color: "#4d6bfe", maskUrl: deepSeekUrl, label: "DeepSeek" },
  copilot: { color: "#6e40c9", maskUrl: githubCopilotUrl, label: "GitHub Copilot" },
  ollama: { color: "#9ca3af", maskUrl: ollamaUrl, label: "Ollama" },
  xiaomi: { color: "#ff6900", maskUrl: xiaomiMiMoUrl, label: "Xiaomi MiMo" },
  kimi: { color: "#7c3aed", maskUrl: kimiUrl, label: "Kimi" },
  foundry: { color: "#0078d4", maskUrl: azureAiUrl, label: "Azure AI Foundry" },
  bedrock: { color: "#ff9900", maskUrl: bedrockUrl, label: "AWS Bedrock" },
  fci: { color: "#f26522", imageUrl: fptUrl, label: "FPT inference gateway" },
  router9: { color: "#60a5fa", label: "Router9" },
  cliproxy: { color: "#f59e0b", label: "CLI proxy" },
}

export const PROVIDER_ALIASES: Record<string, string> = {
  azure: "foundry",
  azureai: "foundry",
  aws: "bedrock",
  gemini: "googlegenai",
  githubcopilot: "copilot",
  google: "googlegenai",
  google_genai: "googlegenai",
  glm: "zai",
  mimo: "xiaomi",
  moonshot: "kimi",
  moonshotai: "kimi",
  vertex: "vertexai",
  xiaomi_mimo: "xiaomi",
}

export const PROVIDER_FALLBACK_COLOR = "#6b7280"
export const PROVIDER_FALLBACK_ID = "unknown"
export const PROVIDER_FALLBACK_LABEL = "Unknown provider"

export const PROVIDER_ICON_SIZE_CLASSES = {
  xs: "size-5 rounded-md",
  sm: "size-8 rounded-lg",
  md: "size-10 rounded-xl",
  lg: "size-12 rounded-xl",
} as const

export const PROVIDER_GLYPH_SIZE_CLASSES = {
  xs: "size-3",
  sm: "size-[1.125rem]",
  md: "size-[1.375rem]",
  lg: "size-[1.625rem]",
} as const

export type ProviderIconSize = keyof typeof PROVIDER_ICON_SIZE_CLASSES
