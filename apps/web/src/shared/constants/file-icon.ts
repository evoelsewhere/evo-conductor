import audioIcon from "material-icon-theme/icons/audio.svg"
import cIcon from "material-icon-theme/icons/c.svg"
import consoleIcon from "material-icon-theme/icons/console.svg"
import cppIcon from "material-icon-theme/icons/cpp.svg"
import cssIcon from "material-icon-theme/icons/css.svg"
import databaseIcon from "material-icon-theme/icons/database.svg"
import dockerIcon from "material-icon-theme/icons/docker.svg"
import drawioIcon from "material-icon-theme/icons/drawio.svg"
import fileIcon from "material-icon-theme/icons/file.svg"
import folderIcon from "material-icon-theme/icons/folder.svg"
import folderOpenIcon from "material-icon-theme/icons/folder-open.svg"
import gitIcon from "material-icon-theme/icons/git.svg"
import goIcon from "material-icon-theme/icons/go.svg"
import htmlIcon from "material-icon-theme/icons/html.svg"
import imageIcon from "material-icon-theme/icons/image.svg"
import javaIcon from "material-icon-theme/icons/java.svg"
import javascriptIcon from "material-icon-theme/icons/javascript.svg"
import jsonIcon from "material-icon-theme/icons/json.svg"
import kotlinIcon from "material-icon-theme/icons/kotlin.svg"
import lessIcon from "material-icon-theme/icons/less.svg"
import licenseIcon from "material-icon-theme/icons/license.svg"
import lockIcon from "material-icon-theme/icons/lock.svg"
import makefileIcon from "material-icon-theme/icons/makefile.svg"
import markdownIcon from "material-icon-theme/icons/markdown.svg"
import nodejsIcon from "material-icon-theme/icons/nodejs.svg"
import npmIcon from "material-icon-theme/icons/npm.svg"
import pdfIcon from "material-icon-theme/icons/pdf.svg"
import phpIcon from "material-icon-theme/icons/php.svg"
import powerpointIcon from "material-icon-theme/icons/powerpoint.svg"
import pythonIcon from "material-icon-theme/icons/python.svg"
import reactIcon from "material-icon-theme/icons/react.svg"
import reactTypescriptIcon from "material-icon-theme/icons/react_ts.svg"
import rubyIcon from "material-icon-theme/icons/ruby.svg"
import rustIcon from "material-icon-theme/icons/rust.svg"
import sassIcon from "material-icon-theme/icons/sass.svg"
import settingsIcon from "material-icon-theme/icons/settings.svg"
import swiftIcon from "material-icon-theme/icons/swift.svg"
import tableIcon from "material-icon-theme/icons/table.svg"
import tomlIcon from "material-icon-theme/icons/toml.svg"
import typescriptIcon from "material-icon-theme/icons/typescript.svg"
import videoIcon from "material-icon-theme/icons/video.svg"
import wordIcon from "material-icon-theme/icons/word.svg"
import xmlIcon from "material-icon-theme/icons/xml.svg"
import yamlIcon from "material-icon-theme/icons/yaml.svg"
import zipIcon from "material-icon-theme/icons/zip.svg"

export const DEFAULT_FILE_ICON = fileIcon
export const FOLDER_ICON = folderIcon
export const FOLDER_OPEN_ICON = folderOpenIcon

export const FILE_ICON_BY_EXTENSION: Readonly<Record<string, string>> = {
  js: javascriptIcon,
  mjs: javascriptIcon,
  cjs: javascriptIcon,
  jsx: reactIcon,
  ts: typescriptIcon,
  mts: typescriptIcon,
  cts: typescriptIcon,
  tsx: reactTypescriptIcon,
  py: pythonIcon,
  pyw: pythonIcon,
  rs: rustIcon,
  go: goIcon,
  java: javaIcon,
  c: cIcon,
  h: cIcon,
  cpp: cppIcon,
  cc: cppIcon,
  cxx: cppIcon,
  hpp: cppIcon,
  php: phpIcon,
  rb: rubyIcon,
  swift: swiftIcon,
  kt: kotlinIcon,
  kts: kotlinIcon,
  html: htmlIcon,
  htm: htmlIcon,
  css: cssIcon,
  scss: sassIcon,
  sass: sassIcon,
  less: lessIcon,
  json: jsonIcon,
  jsonl: jsonIcon,
  ndjson: jsonIcon,
  yaml: yamlIcon,
  yml: yamlIcon,
  toml: tomlIcon,
  xml: xmlIcon,
  svg: imageIcon,
  md: markdownIcon,
  markdown: markdownIcon,
  rst: markdownIcon,
  sql: databaseIcon,
  sqlite: databaseIcon,
  db: databaseIcon,
  sh: consoleIcon,
  bash: consoleIcon,
  zsh: consoleIcon,
  fish: consoleIcon,
  ps1: consoleIcon,
  png: imageIcon,
  jpg: imageIcon,
  jpeg: imageIcon,
  gif: imageIcon,
  webp: imageIcon,
  bmp: imageIcon,
  ico: imageIcon,
  pdf: pdfIcon,
  doc: wordIcon,
  docx: wordIcon,
  xls: tableIcon,
  xlsx: tableIcon,
  csv: tableIcon,
  tsv: tableIcon,
  ppt: powerpointIcon,
  pptx: powerpointIcon,
  zip: zipIcon,
  tar: zipIcon,
  gz: zipIcon,
  tgz: zipIcon,
  rar: zipIcon,
  "7z": zipIcon,
  evoplugin: zipIcon,
  mp3: audioIcon,
  wav: audioIcon,
  flac: audioIcon,
  ogg: audioIcon,
  mp4: videoIcon,
  mov: videoIcon,
  webm: videoIcon,
  mkv: videoIcon,
  drawio: drawioIcon,
  dio: drawioIcon,
}

export const FILE_ICON_SPECIAL_RULES: ReadonlyArray<{
  pattern: RegExp
  icon: string
}> = [
  { pattern: /^readme(?:\.|$)/, icon: markdownIcon },
  { pattern: /^license(?:\.|$)/, icon: licenseIcon },
  { pattern: /^(dockerfile|compose\.ya?ml)$/, icon: dockerIcon },
  { pattern: /^(makefile|gnumakefile|cmakelists\.txt)$/, icon: makefileIcon },
  { pattern: /^package\.json$/, icon: nodejsIcon },
  { pattern: /^package-lock\.json$/, icon: npmIcon },
  { pattern: /^(yarn|pnpm|bun|cargo|uv)\.lock$/, icon: lockIcon },
  { pattern: /^(cargo\.toml|rust-toolchain(?:\.toml)?)$/, icon: rustIcon },
  { pattern: /^(pyproject\.toml|requirements.*\.txt|pipfile)$/, icon: pythonIcon },
  { pattern: /^(\.gitignore|\.gitattributes|\.gitmodules|\.mailmap)$/, icon: gitIcon },
  { pattern: /^(tsconfig|jsconfig).*\.json$/, icon: typescriptIcon },
  {
    pattern: /^(\.env|\.editorconfig|\.npmrc|\.nvmrc|.*config\.(js|ts|json|ya?ml))$/,
    icon: settingsIcon,
  },
]

export const FILE_ICON_BY_MIME_PREFIX: ReadonlyArray<{
  prefix: string
  icon: string
}> = [
  { prefix: "image/", icon: imageIcon },
  { prefix: "audio/", icon: audioIcon },
  { prefix: "video/", icon: videoIcon },
  { prefix: "text/", icon: fileIcon },
]

export const JSON_MIME_ICON = jsonIcon
