#!/bin/bash

# Auto-indexing script for macOS frameworks
# Runs continuously, launching 2 agents at a time until all frameworks are indexed

BATCH_LOG="/workspace/project/framework-index/BATCH-LOG.md"
INDEX_SUMMARY="/workspace/project/framework-index/INDEX-SUMMARY.md"

echo "🚀 Auto-Indexing Started: $(date)"
echo "Target: Index all 2,439 macOS frameworks"
echo ""

# Function to get current progress
get_progress() {
    if [ -f "$INDEX_SUMMARY" ]; then
        grep "Total Frameworks Discovered" "$INDEX_SUMMARY" | grep -oE '[0-9]+' | head -1
    else
        echo "0"
    fi
}

# Function to get current batch number
get_next_batch() {
    if [ -f "$BATCH_LOG" ]; then
        grep "^## Batch" "$BATCH_LOG" | tail -1 | grep -oE '[0-9]+' || echo "0"
    else
        echo "0"
    fi
}

ITERATION=1
TARGET=2439

while true; do
    CURRENT=$(get_progress)
    NEXT_BATCH=$(get_next_batch)

    if [ "$CURRENT" -ge "$TARGET" ]; then
        echo "✅ COMPLETE! Indexed $CURRENT frameworks"
        break
    fi

    echo "----------------------------------------"
    echo "Iteration $ITERATION"
    echo "Progress: $CURRENT / $TARGET frameworks"
    echo "Starting from Batch $NEXT_BATCH"
    echo "----------------------------------------"

    # This would normally launch agents, but we'll do it from the main session
    echo "⏳ Waiting for agents to complete..."

    # Sleep for 5 minutes between checks
    sleep 300

    ITERATION=$((ITERATION + 1))
done

echo ""
echo "🎉 Framework indexing complete!"
echo "Final count: $(get_progress) frameworks"
