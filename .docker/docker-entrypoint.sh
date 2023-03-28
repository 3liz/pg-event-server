#!/bin/bash

set -e;

exec pg-event-server $@
