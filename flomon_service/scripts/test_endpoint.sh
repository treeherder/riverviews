#!/bin/bash
# Test the HTTP endpoint

BASE_URL="http://localhost:8080"

echo "Testing Flood Monitoring Service HTTP Endpoint"
echo "=============================================="
echo

# Test health check
echo "1. Health check:"
echo "   GET $BASE_URL/health"
curl -s "$BASE_URL/health" | jq
echo
echo

# Test site query for Kingston Mines
echo "2. Query Kingston Mines (05568500):"
echo "   GET $BASE_URL/site/05568500"
curl -s "$BASE_URL/site/05568500" | jq
echo
echo

# Test site query for Peoria
echo "3. Query Peoria Pool (05567500):"
echo "   GET $BASE_URL/site/05567500"
curl -s "$BASE_URL/site/05567500" | jq
echo
echo

# Test invalid site
echo "4. Query invalid site (should return 404):"
echo "   GET $BASE_URL/site/INVALID"
curl -s "$BASE_URL/site/INVALID" | jq
echo
echo

# Test invalid endpoint
echo "5. Query invalid endpoint (should return 404):"
echo "   GET $BASE_URL/invalid"
curl -s "$BASE_URL/invalid" | jq
echo
