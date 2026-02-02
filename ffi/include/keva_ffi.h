#ifndef KEVA_FFI_H
#define KEVA_FFI_H

#include <stdint.h>

/// Start the worker thread.
///
/// data_dir: UTF-8 null-terminated path to the data directory.
/// callback: Called from the worker thread with JSON response (null-terminated UTF-8).
/// context: Opaque pointer passed through to callback.
///
/// Returns an opaque handle. Must be freed with keva_worker_stop().
void *keva_worker_start(const char *data_dir,
                        void (*callback)(void *context, const char *json),
                        void *context);

/// Send a JSON request to the worker thread.
///
/// json: Null-terminated UTF-8 JSON string.
void keva_worker_send(void *handle, const char *json);

/// Stop the worker thread and free the handle.
///
/// Sends shutdown and blocks until the worker thread exits.
void keva_worker_stop(void *handle);

#endif
