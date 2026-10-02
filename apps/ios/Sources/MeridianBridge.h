#ifndef MERIDIAN_BRIDGE_H
#define MERIDIAN_BRIDGE_H
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
uint64_t meridian_session_create(const uint8_t *data, size_t length);
char *meridian_session_call(uint64_t handle, const uint8_t *data, size_t length);
void meridian_session_destroy(uint64_t handle);
void meridian_string_free(char *data);
bool meridian_reflect_local_tcp(uint8_t *bytes, size_t length, uint32_t interface_address, uint32_t peer_address);
#endif
