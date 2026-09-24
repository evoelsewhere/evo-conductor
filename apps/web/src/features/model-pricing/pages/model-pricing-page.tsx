import { useMemo, useState } from "react";

import { useQuery } from "@tanstack/react-query";

import { api } from "@/shared/api/client";
import { PageFrame } from "@/shared/components/page-frame";
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading";
import { Badge } from "@/shared/ui/badge";
import { Button } from "@/shared/ui/button";
import { Card, CardContent, CardFooter } from "@/shared/ui/card";
import { Input } from "@/shared/ui/input";
import { Select, type SelectOption } from "@/shared/ui/select";
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table";

const MODEL_CATALOG_KEY = ["model-pricing-catalog"];

const PAGE_SIZE_OPTIONS: SelectOption<string>[] = [
  { value: "20", label: "20 / page" },
  { value: "50", label: "50 / page" },
  { value: "100", label: "100 / page" },
  { value: "0", label: "All" },
];

const PRICING_STATUS_OPTIONS: SelectOption<string>[] = [
  { value: "all", label: "All" },
  { value: "priced", label: "Priced" },
  { value: "unpriced", label: "Unpriced" },
];

type SortColumn =
  | "provider"
  | "model"
  | "input"
  | "output"
  | "cache_read"
  | "cache_write"
  | "reasoning"
  | "tiers";
type SortDirection = "asc" | "desc";

function formatRate(value: number | null): string {
  if (value === null || value === undefined) {
    return "—";
  }
  return `$${(value / 1_000_000).toFixed(2)}/M`;
}

function hasAnyRate(entry: {
  pricing: {
    base: {
      input: number | null;
      output: number | null;
      cache_read: number | null;
      cache_write: number | null;
      reasoning: number | null;
    };
  };
}) {
  const base = entry.pricing.base;
  return (
    base.input !== null ||
    base.output !== null ||
    base.cache_read !== null ||
    base.cache_write !== null ||
    base.reasoning !== null
  );
}

export function ModelPricingPage() {
  const { data, isLoading, error } = useQuery({
    queryKey: MODEL_CATALOG_KEY,
    queryFn: () => api.modelPricingCatalog(),
  });

  const [searchQuery, setSearchQuery] = useState("");
  const [sortColumn, setSortColumn] = useState<SortColumn>("provider");
  const [sortDirection, setSortDirection] = useState<SortDirection>("asc");
  const [pageSize, setPageSize] = useState<string>("20");
  const [currentPage, setCurrentPage] = useState(1);
  const [providerFilter, setProviderFilter] = useState<string>("all");
  const [pricingStatusFilter, setPricingStatusFilter] = useState<string>("all");

  const initialLoading = useMinimumLoading(isLoading);
  const entries = useMemo(() => (Array.isArray(data) ? data : []), [data]);

  const providers = useMemo(() => {
    const unique = new Set(entries.map((entry) => entry.provider));
    return Array.from(unique).sort((a, b) => a.localeCompare(b));
  }, [entries]);

  const normalizedPageSize = Number(pageSize);

  const filtered = useMemo(() => {
    const normalizedQuery = searchQuery.trim().toLowerCase();
    return entries.filter((entry) => {
      const matchesSearch =
        !normalizedQuery ||
        entry.provider.toLowerCase().includes(normalizedQuery) ||
        entry.model.toLowerCase().includes(normalizedQuery);
      const matchesProvider =
        providerFilter === "all" || entry.provider === providerFilter;
      const matchesPricingStatus =
        pricingStatusFilter === "all" ||
        (pricingStatusFilter === "priced"
          ? hasAnyRate(entry)
          : !hasAnyRate(entry));
      return matchesSearch && matchesProvider && matchesPricingStatus;
    });
  }, [entries, searchQuery, providerFilter, pricingStatusFilter]);

  const sorted = useMemo(() => {
    const sortedEntries = [...filtered];
    sortedEntries.sort((a, b) => {
      let aValue: string | number;
      let bValue: string | number;
      if (sortColumn === "provider") {
        aValue = a.provider.toLowerCase();
        bValue = b.provider.toLowerCase();
      } else if (sortColumn === "model") {
        aValue = a.model.toLowerCase();
        bValue = b.model.toLowerCase();
      } else if (sortColumn === "tiers") {
        aValue =
          (a.pricing.tiers ?? []).length +
          (a.pricing.service_tiers ?? []).length;
        bValue =
          (b.pricing.tiers ?? []).length +
          (b.pricing.service_tiers ?? []).length;
      } else {
        const baseA = a.pricing.base;
        const baseB = b.pricing.base;
        const mapA: Record<string, number | null> = {
          input: baseA.input,
          output: baseA.output,
          cache_read: baseA.cache_read,
          cache_write: baseA.cache_write,
          reasoning: baseA.reasoning,
        };
        const mapB: Record<string, number | null> = {
          input: baseB.input,
          output: baseB.output,
          cache_read: baseB.cache_read,
          cache_write: baseB.cache_write,
          reasoning: baseB.reasoning,
        };
        aValue = mapA[sortColumn] ?? 0;
        bValue = mapB[sortColumn] ?? 0;
      }
      if (typeof aValue === "string" && typeof bValue === "string") {
        return sortDirection === "asc"
          ? aValue.localeCompare(bValue)
          : bValue.localeCompare(aValue);
      }
      const comparison = (aValue as number) - (bValue as number);
      return sortDirection === "asc" ? comparison : -comparison;
    });
    return sortedEntries;
  }, [filtered, sortColumn, sortDirection]);

  const totalPages = useMemo(() => {
    if (normalizedPageSize === 0) return 1;
    return Math.max(1, Math.ceil(sorted.length / normalizedPageSize));
  }, [sorted.length, normalizedPageSize]);

  const pageEntries = useMemo(() => {
    if (normalizedPageSize === 0) return sorted;
    const start = (currentPage - 1) * normalizedPageSize;
    return sorted.slice(start, start + normalizedPageSize);
  }, [sorted, currentPage, normalizedPageSize]);

  const hasTiers = filtered.some(
    (entry) => (entry.pricing.tiers ?? []).length > 0,
  );
  const hasServiceTiers = filtered.some(
    (entry) => (entry.pricing.service_tiers ?? []).length > 0,
  );

  const clearFilters = () => {
    setSearchQuery("");
    setProviderFilter("all");
    setPricingStatusFilter("all");
    setCurrentPage(1);
  };

  const hasActiveFilters =
    searchQuery.trim() !== "" ||
    providerFilter !== "all" ||
    pricingStatusFilter !== "all";

  if (initialLoading) {
    return (
      <PageFrame
        title="Model pricing"
        subtitle="Current rates from the synced models.dev catalog."
      >
        <Card>
          <CardContent>
            <TableWrap>
              <Table>
                <TableHead>
                  <TableRow>
                    <TableTh>Provider</TableTh>
                    <TableTh>Model</TableTh>
                    <TableTh className="text-right">Input</TableTh>
                    <TableTh className="text-right">Output</TableTh>
                    <TableTh className="text-right">Cache read</TableTh>
                    <TableTh className="text-right">Cache write</TableTh>
                    <TableTh className="text-right">Reasoning</TableTh>
                    {(hasTiers || hasServiceTiers) && (
                      <TableTh className="text-center">Tiers</TableTh>
                    )}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {Array.from({ length: 8 }).map((_, index) => (
                    <TableRow key={index}>
                      <TableTd colSpan={hasTiers || hasServiceTiers ? 8 : 7}>
                        <div className="h-4 w-full animate-pulse rounded bg-(--bg-key)" />
                      </TableTd>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableWrap>
          </CardContent>
        </Card>
      </PageFrame>
    );
  }

  if (error) {
    return (
      <PageFrame
        title="Model pricing"
        subtitle="Current rates from the synced models.dev catalog."
      >
        <Card>
          <CardContent>
            <div className="rounded-lg border border-(--border-soft) bg-(--bg-key)/55 px-3 py-2 text-sm text-(--color-text-muted)">
              Failed to load model catalog
            </div>
          </CardContent>
        </Card>
      </PageFrame>
    );
  }

  const handleSort = (column: SortColumn) => {
    if (sortColumn === column) {
      setSortDirection((current) => (current === "asc" ? "desc" : "asc"));
    } else {
      setSortColumn(column);
      setSortDirection("asc");
    }
  };

  const handlePageSizeChange = (next: string) => {
    setPageSize(next);
    setCurrentPage(1);
  };

  const totalFiltered = filtered.length;
  const totalEntries = entries.length;
  const showingFrom =
    totalFiltered === 0 ? 0 : (currentPage - 1) * normalizedPageSize + 1;
  const showingTo =
    normalizedPageSize === 0
      ? totalFiltered
      : Math.min(currentPage * normalizedPageSize, totalFiltered);

  return (
    <PageFrame
      title="Model pricing"
      subtitle="Current rates from the synced models.dev catalog."
    >
      <Card className="mb-4">
        <CardContent className="flex flex-row flex-wrap items-center gap-2">
          <Select
            className="w-40"
            value={providerFilter}
            onValueChange={(next) => {
              setProviderFilter(next);
              setCurrentPage(1);
            }}
            options={[
              {
                value: "all",
                label: "All providers",
                ...(providers.length > 0 ? {} : { disabled: true }),
              },
              ...providers.map((provider) => ({
                value: provider,
                label: provider,
              })),
            ]}
            aria-label="Filter by provider"
          />
          <Select
            className="w-40"
            value={pricingStatusFilter}
            onValueChange={(next) => {
              setPricingStatusFilter(next);
              setCurrentPage(1);
            }}
            options={PRICING_STATUS_OPTIONS}
            aria-label="Filter by pricing status"
          />
          <Input
            className="w-64"
            placeholder="Search provider or model"
            value={searchQuery}
            onChange={(event) => {
              setSearchQuery(event.target.value);
              setCurrentPage(1);
            }}
          />
          {hasActiveFilters && (
            <Button variant="ghost" size="sm" onClick={clearFilters}>
              Clear
            </Button>
          )}
        </CardContent>
      </Card>
      <Card>
        <CardContent className="p-0">
          <TableWrap>
            <Table>
              <TableHead>
                <TableRow>
                  <TableTh>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-auto p-0 font-medium transition-colors"
                      onClick={() => handleSort("provider")}
                    >
                      Provider
                      {sortColumn === "provider"
                        ? sortDirection === "asc"
                          ? " ↑"
                          : " ↓"
                        : ""}
                    </Button>
                  </TableTh>
                  <TableTh>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-auto p-0 font-medium transition-colors"
                      onClick={() => handleSort("model")}
                    >
                      Model
                      {sortColumn === "model"
                        ? sortDirection === "asc"
                          ? " ↑"
                          : " ↓"
                        : ""}
                    </Button>
                  </TableTh>
                  <TableTh className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-auto p-0 font-medium transition-colors"
                      onClick={() => handleSort("input")}
                    >
                      Input
                      {sortColumn === "input"
                        ? sortDirection === "asc"
                          ? " ↑"
                          : " ↓"
                        : ""}
                    </Button>
                  </TableTh>
                  <TableTh className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-auto p-0 font-medium transition-colors"
                      onClick={() => handleSort("output")}
                    >
                      Output
                      {sortColumn === "output"
                        ? sortDirection === "asc"
                          ? " ↑"
                          : " ↓"
                        : ""}
                    </Button>
                  </TableTh>
                  <TableTh className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-auto p-0 font-medium transition-colors"
                      onClick={() => handleSort("cache_read")}
                    >
                      Cache read
                      {sortColumn === "cache_read"
                        ? sortDirection === "asc"
                          ? " ↑"
                          : " ↓"
                        : ""}
                    </Button>
                  </TableTh>
                  <TableTh className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-auto p-0 font-medium transition-colors"
                      onClick={() => handleSort("cache_write")}
                    >
                      Cache write
                      {sortColumn === "cache_write"
                        ? sortDirection === "asc"
                          ? " ↑"
                          : " ↓"
                        : ""}
                    </Button>
                  </TableTh>
                  <TableTh className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-auto p-0 font-medium transition-colors"
                      onClick={() => handleSort("reasoning")}
                    >
                      Reasoning
                      {sortColumn === "reasoning"
                        ? sortDirection === "asc"
                          ? " ↑"
                          : " ↓"
                        : ""}
                    </Button>
                  </TableTh>
                  {(hasTiers || hasServiceTiers) && (
                    <TableTh className="text-center">
                      <span className="text-xs font-medium">Tiers</span>
                    </TableTh>
                  )}
                </TableRow>
              </TableHead>
              <TableBody>
                {pageEntries.length === 0 ? (
                  <TableRow>
                    <TableTd
                      colSpan={hasTiers || hasServiceTiers ? 8 : 7}
                      className="text-center text-sm text-(--color-text-muted)"
                    >
                      No models matched your filters.
                    </TableTd>
                  </TableRow>
                ) : (
                  pageEntries.map((entry) => {
                    const base = entry.pricing.base;
                    return (
                      <TableRow
                        key={`${entry.provider}:${entry.model}`}
                        className="transition-colors"
                      >
                        <TableTd className="font-medium">
                          {entry.provider}
                        </TableTd>
                        <TableTd className="font-mono text-xs">
                          {entry.model}
                        </TableTd>
                        <TableTd className="text-right tabular-nums">
                          {formatRate(base.input)}
                        </TableTd>
                        <TableTd className="text-right tabular-nums">
                          {formatRate(base.output)}
                        </TableTd>
                        <TableTd className="text-right tabular-nums">
                          {formatRate(base.cache_read)}
                        </TableTd>
                        <TableTd className="text-right tabular-nums">
                          {formatRate(base.cache_write)}
                        </TableTd>
                        <TableTd className="text-right tabular-nums">
                          {formatRate(base.reasoning)}
                        </TableTd>
                        {(hasTiers || hasServiceTiers) && (
                          <TableTd className="text-center">
                            <div className="flex flex-wrap justify-center gap-1">
                              {(entry.pricing.tiers ?? []).length > 0 && (
                                <Badge
                                  tone="neutral"
                                  className="font-mono text-[10px]"
                                >
                                  {(entry.pricing.tiers ?? []).length} tier
                                  {(entry.pricing.tiers ?? []).length === 1
                                    ? ""
                                    : "s"}
                                </Badge>
                              )}
                              {(entry.pricing.service_tiers ?? []).length >
                                0 && (
                                <Badge
                                  tone="neutral"
                                  className="font-mono text-[10px]"
                                >
                                  {(entry.pricing.service_tiers ?? []).length}{" "}
                                  lane
                                  {(entry.pricing.service_tiers ?? [])
                                    .length === 1
                                    ? ""
                                    : "s"}
                                </Badge>
                              )}
                            </div>
                          </TableTd>
                        )}
                      </TableRow>
                    );
                  })
                )}
              </TableBody>
            </Table>
          </TableWrap>
        </CardContent>
        {totalFiltered > 0 && (
          <CardFooter className="flex flex-col items-center justify-between gap-2 sm:flex-row">
            <div className="flex w-full flex-wrap items-center justify-between gap-3 text-xs text-(--color-text-muted)">
              <div>
                {hasActiveFilters ? (
                  <>
                    Showing {showingFrom}-{showingTo} of {totalFiltered} matched
                    model{totalFiltered === 1 ? "" : "s"} ({totalEntries} total)
                  </>
                ) : (
                  <>
                    Showing {showingFrom}-{showingTo} of {totalFiltered} model
                    {totalFiltered === 1 ? "" : "s"}
                  </>
                )}
              </div>
              <div className="flex items-center gap-2">
                <Select
                  value={String(pageSize)}
                  onValueChange={handlePageSizeChange}
                  options={PAGE_SIZE_OPTIONS}
                  aria-label="Page size"
                />
                <div className="flex items-center gap-1">
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={currentPage === 1}
                    onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                  >
                    Previous
                  </Button>
                  <span className="tabular-nums">
                    Page {currentPage} of {totalPages}
                  </span>
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={currentPage === totalPages}
                    onClick={() =>
                      setCurrentPage((p) => Math.min(totalPages, p + 1))
                    }
                  >
                    Next
                  </Button>
                </div>
              </div>
            </div>
          </CardFooter>
        )}
      </Card>
    </PageFrame>
  );
}
