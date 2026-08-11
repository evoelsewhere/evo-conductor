import {
  RESOURCE_STUDIO_CHANGE_KIND,
  RESOURCE_STUDIO_DIFF,
  type ResourceStudioChangeKind,
} from "@/shared/constants/resource-studio"

export type ResourceLineChange = {
  kind: ResourceStudioChangeKind
  startLine: number
  endLine: number
}

export type ResourceLineDiff = {
  ranges: ResourceLineChange[]
  addedLines: number
  modifiedLines: number
  removedLines: number
  changedLines: number
}

type Match = {
  originalIndex: number
  modifiedIndex: number
}

export function resourceLineDiff(original: string, modified: string): ResourceLineDiff {
  if (original === modified) return emptyDiff()

  const originalLines = splitLines(original)
  const modifiedLines = splitLines(modified)
  let prefix = 0
  while (
    prefix < originalLines.length &&
    prefix < modifiedLines.length &&
    originalLines[prefix] === modifiedLines[prefix]
  ) {
    prefix += 1
  }

  let suffix = 0
  while (
    suffix < originalLines.length - prefix &&
    suffix < modifiedLines.length - prefix &&
    originalLines[originalLines.length - 1 - suffix] ===
      modifiedLines[modifiedLines.length - 1 - suffix]
  ) {
    suffix += 1
  }

  const originalEnd = originalLines.length - suffix
  const modifiedEnd = modifiedLines.length - suffix
  const originalMiddle = originalLines.slice(prefix, originalEnd)
  const modifiedMiddle = modifiedLines.slice(prefix, modifiedEnd)
  const matrixCells = (originalMiddle.length + 1) * (modifiedMiddle.length + 1)
  const matches =
    matrixCells <= RESOURCE_STUDIO_DIFF.MAX_MATRIX_CELLS
      ? longestCommonSubsequence(originalMiddle, modifiedMiddle, prefix)
      : []

  const result = emptyDiff()
  let originalCursor = prefix
  let modifiedCursor = prefix
  for (const match of matches) {
    appendChangedBlock(
      result,
      originalCursor,
      match.originalIndex,
      modifiedCursor,
      match.modifiedIndex,
      modifiedLines.length,
    )
    originalCursor = match.originalIndex + 1
    modifiedCursor = match.modifiedIndex + 1
  }
  appendChangedBlock(
    result,
    originalCursor,
    originalEnd,
    modifiedCursor,
    modifiedEnd,
    modifiedLines.length,
  )
  result.ranges = mergeAdjacentRanges(result.ranges)
  result.changedLines = result.addedLines + result.modifiedLines + result.removedLines
  return result
}

function longestCommonSubsequence(
  original: string[],
  modified: string[],
  offset: number,
): Match[] {
  const columns = modified.length + 1
  const matrix = new Uint32Array((original.length + 1) * columns)
  for (let originalIndex = original.length - 1; originalIndex >= 0; originalIndex -= 1) {
    for (let modifiedIndex = modified.length - 1; modifiedIndex >= 0; modifiedIndex -= 1) {
      const index = originalIndex * columns + modifiedIndex
      matrix[index] =
        original[originalIndex] === modified[modifiedIndex]
          ? matrix[(originalIndex + 1) * columns + modifiedIndex + 1] + 1
          : Math.max(
              matrix[(originalIndex + 1) * columns + modifiedIndex],
              matrix[originalIndex * columns + modifiedIndex + 1],
            )
    }
  }

  const matches: Match[] = []
  let originalIndex = 0
  let modifiedIndex = 0
  while (originalIndex < original.length && modifiedIndex < modified.length) {
    if (original[originalIndex] === modified[modifiedIndex]) {
      matches.push({
        originalIndex: originalIndex + offset,
        modifiedIndex: modifiedIndex + offset,
      })
      originalIndex += 1
      modifiedIndex += 1
    } else if (
      matrix[(originalIndex + 1) * columns + modifiedIndex] >=
      matrix[originalIndex * columns + modifiedIndex + 1]
    ) {
      originalIndex += 1
    } else {
      modifiedIndex += 1
    }
  }
  return matches
}

function appendChangedBlock(
  result: ResourceLineDiff,
  originalStart: number,
  originalEnd: number,
  modifiedStart: number,
  modifiedEnd: number,
  modifiedLineCount: number,
) {
  const originalCount = originalEnd - originalStart
  const modifiedCount = modifiedEnd - modifiedStart
  if (originalCount === 0 && modifiedCount === 0) return

  const paired = Math.min(originalCount, modifiedCount)
  if (paired > 0) {
    result.ranges.push({
      kind: RESOURCE_STUDIO_CHANGE_KIND.MODIFIED,
      startLine: modifiedStart + 1,
      endLine: modifiedStart + paired,
    })
    result.modifiedLines += paired
  }
  if (modifiedCount > paired) {
    result.ranges.push({
      kind: RESOURCE_STUDIO_CHANGE_KIND.ADDED,
      startLine: modifiedStart + paired + 1,
      endLine: modifiedEnd,
    })
    result.addedLines += modifiedCount - paired
  }
  if (originalCount > paired) {
    const removed = originalCount - paired
    result.ranges.push({
      kind: RESOURCE_STUDIO_CHANGE_KIND.DELETED,
      startLine: deletionAnchor(modifiedStart + paired, modifiedLineCount),
      endLine: deletionAnchor(modifiedStart + paired, modifiedLineCount),
    })
    result.removedLines += removed
  }
}

function deletionAnchor(index: number, lineCount: number): number {
  return Math.min(Math.max(index + 1, 1), Math.max(lineCount, 1))
}

function mergeAdjacentRanges(ranges: ResourceLineChange[]): ResourceLineChange[] {
  return ranges.reduce<ResourceLineChange[]>((merged, range) => {
    const previous = merged.at(-1)
    if (
      previous &&
      previous.kind === range.kind &&
      range.startLine <= previous.endLine + 1
    ) {
      previous.endLine = Math.max(previous.endLine, range.endLine)
      return merged
    }
    merged.push({ ...range })
    return merged
  }, [])
}

function splitLines(content: string): string[] {
  if (!content) return []
  const lines = content.replace(/\r\n/g, "\n").split("\n")
  if (lines.at(-1) === "") lines.pop()
  return lines
}

function emptyDiff(): ResourceLineDiff {
  return {
    ranges: [],
    addedLines: 0,
    modifiedLines: 0,
    removedLines: 0,
    changedLines: 0,
  }
}
