#include <cstdlib>
#include <string>

#ifndef RLSHIM_CURL_H
#define RLSHIM_CURL_H

inline size_t rlshim_write_callback(char* ptr, size_t size, size_t nmemb, void* userdata) {
    auto* body = static_cast<std::string*>(userdata);
    body->append(ptr, size * nmemb);
    return size * nmemb;
}

#endif  // RLSHIM_CURL_H