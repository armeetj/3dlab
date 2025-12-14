#!/bin/bash
# Run both server and client for 3DLab

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Set HDF5 path for macOS
export HDF5_DIR=/opt/homebrew/opt/hdf5

# Kill any existing processes
pkill -f "target/.*/server" 2>/dev/null
pkill -f "trunk serve" 2>/dev/null

echo -e "${BLUE}Starting 3DLab...${NC}"
echo ""

# Start server in background
echo -e "${GREEN}Starting server on http://localhost:3000${NC}"
cargo run -p server &
SERVER_PID=$!

# Wait for server to start
sleep 2

# Start trunk serve in background
echo -e "${GREEN}Starting client on http://localhost:8080${NC}"
cd client && trunk serve &
TRUNK_PID=$!
cd ..

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}3DLab is running!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "  Server API:  http://localhost:3000"
echo "  Client App:  http://localhost:8080"
echo ""
echo "Press Ctrl+C to stop both services"
echo ""

# Handle Ctrl+C to kill both processes
trap "echo ''; echo 'Stopping services...'; kill $SERVER_PID $TRUNK_PID 2>/dev/null; exit 0" INT

# Wait for either process to exit
wait
