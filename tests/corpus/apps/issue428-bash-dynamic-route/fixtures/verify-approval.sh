#!/bin/sh
# A readable route must still meet the registered expense approval predicate.
grep -q 'status: "approved"' 'src/app/api/expenses/[id]/approve/route.ts'
