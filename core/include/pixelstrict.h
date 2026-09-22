#ifndef PIXELSTRICT_H
#define PIXELSTRICT_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct PsResult PsResult;
uint8_t *ps_alloc(size_t len);
void ps_dealloc(uint8_t *data, size_t len);
PsResult *ps_convert(const uint8_t *data, size_t len, const uint8_t *json, size_t json_len);
const uint8_t *ps_result_json(const PsResult *result);
size_t ps_result_json_len(const PsResult *result);
const uint8_t *ps_result_png(const PsResult *result);
size_t ps_result_png_len(const PsResult *result);
void ps_result_free(PsResult *result);
#ifdef __cplusplus
}
#endif
#endif
